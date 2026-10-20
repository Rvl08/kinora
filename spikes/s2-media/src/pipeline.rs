//! One export: decode -> (optional GPU composite) -> encode -> mux.
//!
//! Modes (the question S2 answers is what each costs):
//! - `Sw`: software decode, frames in system memory.
//! - `HwDownload`: D3D11VA/CUDA decode, frames copied to system memory.
//! - `ZeroCopy`: hardware frames handed straight to a hardware encoder.
//! - `Composite`: hardware decode, then wgpu (YUV->RGB, shared WGSL effects,
//!   RGB->YUV) and read back: the shape of a real Kinora export today.

use std::time::Instant;

use ffmpeg::ffi::*;
use ffmpeg::{
    Dictionary, Packet, Rational, Rescale, codec, encoder, format, frame, media, software::scaling,
};
use ffmpeg_next as ffmpeg;
use serde::Serialize;

use crate::audio::AudioTrack;
use crate::composite::Compositor;
use crate::hw::{self, Device, EncoderFields};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Mode {
    Sw,
    HwDownload,
    ZeroCopy,
    Composite,
}

pub struct Job<'a> {
    pub input: &'a str,
    pub output: &'a str,
    pub encoder: &'a str,
    pub mode: Mode,
    pub device: Device,
    pub bitrate: i64,
    pub gop: i32,
    pub b_frames: Option<i32>,
    pub max_frames: Option<usize>,
    pub scale: Option<(u32, u32)>,
    pub options: Vec<(&'a str, &'a str)>,
    pub keep_10bit: bool,
    pub audio: Option<AudioTrack>,
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub encoder: String,
    pub mode: String,
    pub frames: usize,
    pub wall_s: f64,
    pub media_s: f64,
    pub fps: f64,
    pub x_realtime: f64,
    pub bytes: u64,
    pub bitrate_mbps: f64,
    pub cpu_pct_one_core: f32,
    pub decoded_format: String,
    pub encoder_format: String,
    pub size: String,
    pub error: Option<String>,
}

fn container_for(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("webm") => "webm",
        Some("mov") => "mov",
        Some("mkv") => "matroska",
        _ => "mp4",
    }
}

/// The pixel format each encoder takes from system memory.
fn encoder_format(name: &str, source_10bit: bool, keep_10bit: bool) -> AVPixelFormat {
    use AVPixelFormat::*;
    let ten = source_10bit && keep_10bit;
    match name {
        n if n.starts_with("h264_") => AV_PIX_FMT_NV12,
        n if n.ends_with("_nvenc") || n.ends_with("_qsv") || n.ends_with("_amf") => {
            if ten {
                AV_PIX_FMT_P010LE
            } else {
                AV_PIX_FMT_NV12
            }
        }
        "libsvtav1" => {
            if ten {
                AV_PIX_FMT_YUV420P10LE
            } else {
                AV_PIX_FMT_YUV420P
            }
        }
        "prores_ks" => AV_PIX_FMT_YUV422P10LE,
        "dnxhd" => AV_PIX_FMT_YUV422P,
        _ => AV_PIX_FMT_YUV420P,
    }
}

fn is_10bit(fmt: i32) -> bool {
    use AVPixelFormat::*;
    [
        AV_PIX_FMT_P010LE,
        AV_PIX_FMT_YUV420P10LE,
        AV_PIX_FMT_YUV422P10LE,
        AV_PIX_FMT_YUV444P10LE,
    ]
    .iter()
    .any(|f| *f as i32 == fmt)
}

struct Encoding {
    enc: encoder::Video,
    tb: Rational,
    scaler: Option<scaling::Context>,
    target: AVPixelFormat,
}

pub fn transcode(job: &Job, compositor: &mut Option<Compositor>) -> Outcome {
    let mut out = Outcome {
        encoder: job.encoder.into(),
        mode: format!("{:?}", job.mode),
        ..Default::default()
    };
    match run(job, compositor, &mut out) {
        Ok(()) => {}
        Err(e) => out.error = Some(e),
    }
    out
}

fn run(job: &Job, compositor: &mut Option<Compositor>, out: &mut Outcome) -> Result<(), String> {
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::get_current_pid()?;
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    let cpu0 = sys
        .process(pid)
        .map(|p| p.accumulated_cpu_time())
        .unwrap_or(0);

    let codec = encoder::find_by_name(job.encoder)
        .ok_or_else(|| format!("encoder {} not in this FFmpeg build", job.encoder))?;
    let mut ictx = format::input(job.input).map_err(|e| format!("open input: {e}"))?;
    let ist = ictx
        .streams()
        .best(media::Type::Video)
        .ok_or("no video stream")?;
    let ist_index = ist.index();
    let in_tb = ist.time_base();
    let rate = ist.avg_frame_rate();
    // Encoders get 1/fps as their time base: several software encoders (OpenH264,
    // SVT-AV1, libvpx) derive the frame rate for rate control from it, and a
    // container time base such as Matroska's 1/1000 made them overshoot 3-45x.
    let enc_tb = if rate.numerator() > 0 && rate.denominator() > 0 {
        rate.invert()
    } else {
        in_tb
    };
    let mut dctx =
        codec::context::Context::from_parameters(ist.parameters()).map_err(|e| e.to_string())?;
    let hw_decode = job.mode != Mode::Sw;
    unsafe {
        let c = dctx.as_mut_ptr();
        (*c).thread_count = 0;
        if hw_decode {
            hw::attach_device(c, job.device)?;
            // Encoders hold on to frames in zero-copy mode: give the pool headroom.
            (*c).extra_hw_frames = 24;
        }
    }
    let mut dec = dctx
        .decoder()
        .video()
        .map_err(|e| format!("open decoder: {e}"))?;
    let color = unsafe {
        let c = dec.as_ptr();
        (
            (*c).color_primaries,
            (*c).color_trc,
            (*c).colorspace,
            (*c).color_range,
        )
    };

    let mut octx = format::output_as(job.output, container_for(job.output))
        .map_err(|e| format!("open output: {e}"))?;
    let global_header = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);

    let mut encoding: Option<Encoding> = None;
    let mut audio_packets: Vec<Packet> = Vec::new();
    let mut audio_tb = Rational(1, 48_000);
    let mut header_written = false;
    let mut next_audio = 0usize;
    let mut frames = 0usize;
    let mut bytes_media = 0u64;
    let mut first_pts: Option<i64> = None;
    let mut last_pts = 0i64;
    let t0 = Instant::now();

    let mut decoded = frame::Video::empty();
    let mut packets = ictx.packets();
    let mut flushing = false;
    'decode: loop {
        if !flushing {
            match packets.next() {
                Some((s, p)) if s.index() == ist_index => {
                    dec.send_packet(&p).map_err(|e| format!("decode: {e}"))?
                }
                Some(_) => continue,
                None => {
                    dec.send_eof().ok();
                    flushing = true;
                }
            }
        }
        let mut got_any = false;
        while dec.receive_frame(&mut decoded).is_ok() {
            got_any = true;
            if job.max_frames.is_some_and(|m| frames >= m) {
                break 'decode;
            }
            let pts = decoded.timestamp().or(decoded.pts()).unwrap_or(0);
            decoded.set_pts(Some(pts));
            first_pts.get_or_insert(pts);
            last_pts = pts;

            // Frame as the encoder will take it.
            let mut work: frame::Video = match job.mode {
                Mode::ZeroCopy | Mode::Sw => hw::frame_ref(&decoded),
                Mode::HwDownload | Mode::Composite => {
                    if hw::is_hw_frame(&decoded) {
                        hw::download(&decoded)?
                    } else {
                        hw::frame_ref(&decoded)
                    }
                }
            };
            if out.decoded_format.is_empty() {
                out.decoded_format = hw::pix_fmt_name(unsafe { (*decoded.as_ptr()).format });
            }
            if job.mode == Mode::Composite {
                let c =
                    compositor.get_or_insert_with(|| Compositor::new(work.width(), work.height()));
                work = c.process(&work)?;
            }

            if encoding.is_none() {
                let source_fmt = unsafe { (*work.as_ptr()).format };
                let (w, h) = job.scale.unwrap_or((work.width(), work.height()));
                let target = if job.mode == Mode::ZeroCopy {
                    job.device.hw_pix_fmt()
                } else {
                    encoder_format(job.encoder, is_10bit(source_fmt), job.keep_10bit)
                };
                let scaler = if job.mode != Mode::ZeroCopy
                    && (source_fmt != target as i32 || job.scale.is_some())
                {
                    Some(
                        scaling::Context::get(
                            format::Pixel::from(unsafe {
                                std::mem::transmute::<i32, AVPixelFormat>(source_fmt)
                            }),
                            work.width(),
                            work.height(),
                            format::Pixel::from(target),
                            w,
                            h,
                            scaling::Flags::BILINEAR,
                        )
                        .map_err(|e| format!("scaler: {e}"))?,
                    )
                } else {
                    None
                };
                let mut ectx = codec::context::Context::new_with_codec(codec);
                let fields = EncoderFields {
                    width: w as i32,
                    height: h as i32,
                    pix_fmt: target,
                    time_base: enc_tb.into(),
                    frame_rate: rate.into(),
                    bit_rate: job.bitrate,
                    vbv: job.encoder.ends_with("_nvenc")
                        || job.encoder.ends_with("_qsv")
                        || job.encoder.ends_with("_amf"),
                    gop: job.gop,
                    max_b_frames: job.b_frames,
                    global_header,
                    color: Some(color),
                    hw_frames_ctx: if job.mode == Mode::ZeroCopy {
                        // The frame's own pool, as the ffmpeg CLI does.
                        let f = unsafe { (*decoded.as_ptr()).hw_frames_ctx };
                        if f.is_null() {
                            return Err("decoder has no hw_frames_ctx".into());
                        }
                        Some(f)
                    } else {
                        None
                    },
                    threads: 0,
                };
                unsafe { hw::apply(ectx.as_mut_ptr(), &fields) };
                let mut opts = Dictionary::new();
                for (k, v) in &job.options {
                    opts.set(k, v);
                }
                let enc = ectx
                    .encoder()
                    .video()
                    .map_err(|e| e.to_string())?
                    .open_with(opts)
                    .map_err(|e| format!("open encoder {}: {e}", job.encoder))?;
                out.encoder_format = hw::pix_fmt_name(target as i32);
                out.size = format!("{w}x{h}");
                let mut ost = octx.add_stream(codec).map_err(|e| e.to_string())?;
                ost.set_parameters(&enc);
                ost.set_time_base(enc_tb);
                if let Some(track) = &job.audio {
                    let (params, tb) = track.stream_params();
                    let mut ast = octx
                        .add_stream(encoder::find(codec::Id::AAC).ok_or("no aac encoder")?)
                        .map_err(|e| e.to_string())?;
                    ast.set_parameters(params);
                    ast.set_time_base(tb);
                    audio_tb = tb;
                }
                octx.write_header()
                    .map_err(|e| format!("write header: {e}"))?;
                header_written = true;
                if let Some(track) = &job.audio {
                    audio_packets = track.packets()?;
                }
                encoding = Some(Encoding {
                    enc,
                    tb: enc_tb,
                    scaler,
                    target,
                });
            }
            let e = encoding.as_mut().unwrap();
            let to_encode = match e.scaler.as_mut() {
                Some(s) => {
                    let mut dst = frame::Video::empty();
                    s.run(&work, &mut dst)
                        .map_err(|er| format!("scale: {er}"))?;
                    dst.set_pts(work.pts());
                    unsafe {
                        let (p, t, sp, r) = color;
                        (*dst.as_mut_ptr()).color_primaries = p;
                        (*dst.as_mut_ptr()).color_trc = t;
                        (*dst.as_mut_ptr()).colorspace = sp;
                        (*dst.as_mut_ptr()).color_range = r;
                    }
                    dst
                }
                None => work,
            };
            let _ = e.target;
            let mut to_encode = to_encode;
            let pts = to_encode.pts().map(|t| t.rescale(in_tb, enc_tb));
            to_encode.set_pts(pts);
            // A decoded frame carries its source picture type and key flag, and
            // encoders take pict_type = I as "force a key frame": an all-intra
            // source (FFV1, ProRes) then becomes all-intra output and blows the
            // bitrate. The ffmpeg CLI clears both; so must Kinora's exporter.
            unsafe {
                let f = to_encode.as_mut_ptr();
                (*f).pict_type = AVPictureType::AV_PICTURE_TYPE_NONE;
                (*f).flags &= !(AV_FRAME_FLAG_KEY as i32);
                (*f).duration = (*f).duration.rescale(in_tb, enc_tb);
            }
            e.enc
                .send_frame(&to_encode)
                .map_err(|er| format!("encode: {er}"))?;
            frames += 1;
            bytes_media += drain(
                &mut e.enc,
                e.tb,
                &mut octx,
                &audio_packets,
                &mut next_audio,
                audio_tb,
            )?;
        }
        if flushing && !got_any {
            break;
        }
    }
    let e = encoding.as_mut().ok_or("no frames decoded")?;
    e.enc.send_eof().ok();
    bytes_media += drain(
        &mut e.enc,
        e.tb,
        &mut octx,
        &audio_packets,
        &mut next_audio,
        audio_tb,
    )?;
    // Any audio past the last video frame.
    while next_audio < audio_packets.len() {
        let mut p = audio_packets[next_audio].clone();
        p.set_stream(1);
        let ost_tb = octx.stream(1).unwrap().time_base();
        p.rescale_ts(audio_tb, ost_tb);
        p.write_interleaved(&mut octx)
            .map_err(|er| er.to_string())?;
        next_audio += 1;
    }
    if header_written {
        octx.write_trailer()
            .map_err(|er| format!("trailer: {er}"))?;
    }
    let wall = t0.elapsed().as_secs_f64();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    // CPU time over wall time: 100 = one core busy for the whole export.
    let cpu1 = sys
        .process(pid)
        .map(|p| p.accumulated_cpu_time())
        .unwrap_or(cpu0);
    out.cpu_pct_one_core = ((cpu1.saturating_sub(cpu0)) as f64 / (wall * 1000.0) * 100.0) as f32;
    let media_s = {
        let span = (last_pts - first_pts.unwrap_or(0)) as f64 * f64::from(in_tb);
        let frame_s = if rate.denominator() != 0 {
            1.0 / f64::from(rate)
        } else {
            0.0
        };
        span + frame_s
    };
    out.frames = frames;
    out.wall_s = wall;
    out.media_s = media_s;
    out.fps = frames as f64 / wall;
    out.x_realtime = media_s / wall;
    out.bytes = std::fs::metadata(job.output).map(|m| m.len()).unwrap_or(0);
    out.bitrate_mbps = bytes_media as f64 * 8.0 / media_s / 1e6;
    Ok(())
}

/// Writes every packet the encoder has ready, interleaving audio up to it.
fn drain(
    enc: &mut encoder::Video,
    tb: Rational,
    octx: &mut format::context::Output,
    audio: &[Packet],
    next_audio: &mut usize,
    audio_tb: Rational,
) -> Result<u64, String> {
    let mut bytes = 0u64;
    let mut packet = Packet::empty();
    while enc.receive_packet(&mut packet).is_ok() {
        let vts = packet.dts().or(packet.pts()).unwrap_or(0);
        let v_sec = vts as f64 * f64::from(tb);
        while *next_audio < audio.len() {
            let a = &audio[*next_audio];
            if a.pts().unwrap_or(0) as f64 * f64::from(audio_tb) > v_sec {
                break;
            }
            let mut p = a.clone();
            p.set_stream(1);
            let ost_tb = octx.stream(1).unwrap().time_base();
            p.rescale_ts(audio_tb, ost_tb);
            p.write_interleaved(octx).map_err(|e| e.to_string())?;
            *next_audio += 1;
        }
        bytes += packet.size() as u64;
        packet.set_stream(0);
        let ost_tb = octx.stream(0).unwrap().time_base();
        packet.rescale_ts(tb, ost_tb);
        packet
            .write_interleaved(octx)
            .map_err(|e| format!("mux: {e}"))?;
        packet = Packet::empty();
    }
    Ok(bytes)
}
