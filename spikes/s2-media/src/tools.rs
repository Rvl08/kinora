//! Encoder probing, thumbnails, OpenH264 through Cisco's DLL, and quality
//! measurement.

use std::process::Command;
use std::time::Instant;

use ffmpeg::ffi::*;
use ffmpeg::{Dictionary, codec, encoder, format, frame, media, software::scaling};
use ffmpeg_next as ffmpeg;
use serde::Serialize;
use serde_json::{Value, json};

use crate::hw::{self, EncoderFields};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub encoder: String,
    pub in_build: bool,
    pub opens: bool,
    pub ms: f64,
    pub error: Option<String>,
}

/// What the app would do at start (on a background thread): which encoders
/// actually open on this machine.
pub fn probe_encoders(names: &[&str]) -> (Vec<Probe>, f64) {
    let t_all = Instant::now();
    let mut out = Vec::new();
    for &name in names {
        let t0 = Instant::now();
        let Some(codec) = encoder::find_by_name(name) else {
            out.push(Probe {
                encoder: name.into(),
                in_build: false,
                opens: false,
                ms: 0.0,
                error: Some("not in build".into()),
            });
            continue;
        };
        let fmt = match name {
            "prores_ks" => AVPixelFormat::AV_PIX_FMT_YUV422P10LE,
            "dnxhd" => AVPixelFormat::AV_PIX_FMT_YUV422P,
            n if n.ends_with("_nvenc") || n.ends_with("_qsv") || n.ends_with("_amf") => {
                AVPixelFormat::AV_PIX_FMT_NV12
            }
            _ => AVPixelFormat::AV_PIX_FMT_YUV420P,
        };
        let mut ctx = codec::context::Context::new_with_codec(codec);
        unsafe {
            hw::apply(
                ctx.as_mut_ptr(),
                &EncoderFields {
                    width: 1920,
                    height: 1080,
                    pix_fmt: fmt,
                    time_base: AVRational { num: 1, den: 30 },
                    frame_rate: AVRational { num: 30, den: 1 },
                    bit_rate: 6_000_000,
                    vbv: false,
                    gop: 60,
                    max_b_frames: None,
                    global_header: false,
                    color: None,
                    hw_frames_ctx: None,
                    threads: 0,
                },
            )
        };
        let mut opts = Dictionary::new();
        if name == "dnxhd" {
            opts.set("profile", "dnxhr_hq");
        }
        let r = ctx.encoder().video().and_then(|v| v.open_with(opts));
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        match r {
            Ok(_) => out.push(Probe {
                encoder: name.into(),
                in_build: true,
                opens: true,
                ms,
                error: None,
            }),
            Err(e) => out.push(Probe {
                encoder: name.into(),
                in_build: true,
                opens: false,
                ms,
                error: Some(e.to_string()),
            }),
        }
    }
    (out, t_all.elapsed().as_secs_f64() * 1000.0)
}

/// `count` evenly spaced 160x90 thumbnails. Key-frame mode decodes only key
/// frames (fast, approximate); exact mode decodes up to each timestamp.
pub fn thumbnails(path: &str, count: usize, keyframes_only: bool) -> Result<Value, String> {
    let t0 = Instant::now();
    let mut ictx = format::input(path).map_err(|e| e.to_string())?;
    let st = ictx.streams().best(media::Type::Video).ok_or("no video")?;
    let index = st.index();
    let tb = st.time_base();
    let duration_us = ictx.duration();
    let mut dctx =
        codec::context::Context::from_parameters(st.parameters()).map_err(|e| e.to_string())?;
    unsafe {
        (*dctx.as_mut_ptr()).thread_count = 0;
        if keyframes_only {
            (*dctx.as_mut_ptr()).skip_frame = AVDiscard::AVDISCARD_NONKEY;
        }
    }
    let mut dec = dctx.decoder().video().map_err(|e| e.to_string())?;
    let mut scaler: Option<scaling::Context> = None;
    let mut made = 0usize;
    let mut max_err_ms = 0f64;
    for i in 0..count {
        let target = duration_us * i as i64 / count as i64;
        ictx.seek(target, ..target).map_err(|e| e.to_string())?;
        dec.flush();
        let mut decoded = frame::Video::empty();
        let mut done = false;
        for (s, p) in ictx.packets() {
            if s.index() != index {
                continue;
            }
            if dec.send_packet(&p).is_err() {
                continue;
            }
            while dec.receive_frame(&mut decoded).is_ok() {
                let pts_us = decoded.timestamp().unwrap_or(0) as f64 * f64::from(tb) * 1e6;
                if !keyframes_only && pts_us + 1.0 < target as f64 {
                    continue; // exact: run up to the target
                }
                max_err_ms = max_err_ms.max((pts_us - target as f64).abs() / 1000.0);
                let sc = scaler.get_or_insert_with(|| {
                    scaling::Context::get(
                        decoded.format(),
                        decoded.width(),
                        decoded.height(),
                        format::Pixel::RGBA,
                        160,
                        90,
                        scaling::Flags::BILINEAR,
                    )
                    .unwrap()
                });
                let mut thumb = frame::Video::empty();
                sc.run(&decoded, &mut thumb).map_err(|e| e.to_string())?;
                made += 1;
                done = true;
                break;
            }
            if done {
                break;
            }
        }
        // Frame threading holds output back; near the end of the file only a flush
        // releases the last key frames.
        if !done {
            dec.send_eof().ok();
            if dec.receive_frame(&mut decoded).is_ok() {
                let pts_us = decoded.timestamp().unwrap_or(0) as f64 * f64::from(tb) * 1e6;
                max_err_ms = max_err_ms.max((pts_us - target as f64).abs() / 1000.0);
                made += 1;
            }
        }
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    Ok(json!({
        "file": path.rsplit(['/', '\\']).next(),
        "mode": if keyframes_only { "keyframes" } else { "exact" },
        "count": count, "made": made, "totalMs": ms, "msPerThumb": ms / count as f64,
        "maxTimestampErrorMs": max_err_ms,
    }))
}

/// H.264 through Cisco's binary OpenH264, loaded at run time (the only way its
/// patent licence covers the codec). Writes an Annex B stream; returns timing.
pub fn openh264_cisco(
    dll: &str,
    input: &str,
    output_h264: &str,
    bitrate: u32,
    max_frames: usize,
) -> Result<Value, String> {
    use openh264::OpenH264API;
    use openh264::encoder::{
        BitRate, Complexity, Encoder, EncoderConfig, FrameRate, RateControlMode, UsageType,
        VuiConfig,
    };
    use openh264::formats::YUVSlices;

    let t_load = Instant::now();
    let api = OpenH264API::from_blob_path(dll).map_err(|e| format!("load {dll}: {e}"))?;
    let load_ms = t_load.elapsed().as_secs_f64() * 1000.0;

    let mut ictx = format::input(input).map_err(|e| e.to_string())?;
    let st = ictx.streams().best(media::Type::Video).ok_or("no video")?;
    let index = st.index();
    let rate = f64::from(st.avg_frame_rate());
    let mut dctx =
        codec::context::Context::from_parameters(st.parameters()).map_err(|e| e.to_string())?;
    unsafe { (*dctx.as_mut_ptr()).thread_count = 0 };
    let mut dec = dctx.decoder().video().map_err(|e| e.to_string())?;

    let config = EncoderConfig::new()
        .bitrate(BitRate::from_bps(bitrate))
        .max_frame_rate(FrameRate::from_hz(rate as f32))
        .rate_control_mode(RateControlMode::Bitrate)
        // Cisco 2.6.0 rejects the non-real-time usage types ("Invalid usage type = 2").
        .usage_type(UsageType::CameraVideoRealTime)
        .complexity(Complexity::High)
        .skip_frames(false)
        .num_threads(0)
        .vui(VuiConfig::bt709());
    let mut enc = Encoder::with_api_config(api, config).map_err(|e| e.to_string())?;
    let mut bitstream = Vec::new();
    let mut frames = 0usize;
    let mut decoded = frame::Video::empty();
    let mut to420: Option<scaling::Context> = None;
    let t0 = Instant::now();
    let mut encode_s = 0f64;
    'outer: for (s, p) in ictx.packets() {
        if s.index() != index {
            continue;
        }
        dec.send_packet(&p).map_err(|e| e.to_string())?;
        while dec.receive_frame(&mut decoded).is_ok() {
            let f = if decoded.format() == format::Pixel::YUV420P {
                decoded.clone()
            } else {
                let sc = to420.get_or_insert_with(|| {
                    scaling::Context::get(
                        decoded.format(),
                        decoded.width(),
                        decoded.height(),
                        format::Pixel::YUV420P,
                        decoded.width(),
                        decoded.height(),
                        scaling::Flags::BILINEAR,
                    )
                    .unwrap()
                });
                let mut o = frame::Video::empty();
                sc.run(&decoded, &mut o).map_err(|e| e.to_string())?;
                o
            };
            let (w, h) = (f.width() as usize, f.height() as usize);
            let yuv = YUVSlices::new(
                (f.data(0), f.data(1), f.data(2)),
                (w, h),
                (f.stride(0), f.stride(1), f.stride(2)),
            );
            let te = Instant::now();
            let bs = enc.encode(&yuv).map_err(|e| e.to_string())?;
            bs.write_vec(&mut bitstream);
            encode_s += te.elapsed().as_secs_f64();
            frames += 1;
            if frames >= max_frames {
                break 'outer;
            }
        }
    }
    let wall = t0.elapsed().as_secs_f64();
    std::fs::write(output_h264, &bitstream).map_err(|e| e.to_string())?;
    let media_s = frames as f64 / rate;
    Ok(json!({
        "encoder": "openh264 (Cisco DLL 2.6.0 via libloading)",
        "dllLoadMs": load_ms,
        "frames": frames,
        "wallS": wall,
        "encodeOnlyS": encode_s,
        "fps": frames as f64 / wall,
        "encodeOnlyFps": frames as f64 / encode_s,
        "xRealtime": media_s / wall,
        "bitrateMbps": bitstream.len() as f64 * 8.0 / media_s / 1e6,
    }))
}

/// VMAF (v0.6.1), PSNR and SSIM of `distorted` against `reference`, through
/// the vendored FFmpeg's libvmaf. Measurement tooling only. Frames are paired
/// by index (setpts=N): timestamp pairing mismatched every third frame when the
/// reference had millisecond timestamps (Matroska) and the output did not.
pub fn quality(
    ffmpeg_exe: &str,
    distorted: &str,
    reference: &str,
    log: &str,
) -> Result<Value, String> {
    let graph = format!(
        "[0:v]settb=1/30,setpts=N,format=yuv420p[d];[1:v]settb=1/30,setpts=N,format=yuv420p[r];\
         [d][r]libvmaf=model=version=vmaf_v0.6.1:feature=name=psnr|name=float_ssim:n_threads=16:log_fmt=json:log_path={}",
        log.replace('\\', "/").replace(':', "\\\\:")
    );
    let out = Command::new(ffmpeg_exe)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            distorted,
            "-i",
            reference,
            "-lavfi",
            &graph,
            "-f",
            "null",
            "-",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "libvmaf failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let text = std::fs::read_to_string(log).map_err(|e| format!("{log}: {e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let pooled = &v["pooled_metrics"];
    let mean = |k: &str| pooled[k]["mean"].as_f64();
    Ok(json!({
        "vmaf": mean("vmaf"),
        "vmafMin": pooled["vmaf"]["min"].as_f64(),
        "psnrY": mean("psnr_y"),
        "ssim": mean("float_ssim"),
    }))
}

/// Remuxes a raw Annex B H.264 stream into MP4 (for quality comparison).
pub fn remux_h264(ffmpeg_exe: &str, raw: &str, fps: f64, out: &str) -> Result<(), String> {
    let s = Command::new(ffmpeg_exe)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-r",
            &format!("{fps}"),
            "-i",
            raw,
            "-c",
            "copy",
            out,
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if s.success() {
        Ok(())
    } else {
        Err("remux failed".into())
    }
}

/// Finds which encoder setting Cisco's DLL rejects: one blank 1080p frame per config.
pub fn openh264_debug(dll: &str) {
    use openh264::OpenH264API;
    use openh264::encoder::{
        BitRate, Complexity, Encoder, EncoderConfig, FrameRate, RateControlMode, UsageType,
        VuiConfig,
    };
    use openh264::formats::YUVBuffer;
    let base = || EncoderConfig::new();
    let configs: Vec<(&str, EncoderConfig)> = vec![
        ("default", base()),
        ("+bitrate", base().bitrate(BitRate::from_bps(6_000_000))),
        (
            "+framerate",
            base()
                .bitrate(BitRate::from_bps(6_000_000))
                .max_frame_rate(FrameRate::from_hz(30.0)),
        ),
        (
            "+rc bitrate",
            base()
                .bitrate(BitRate::from_bps(6_000_000))
                .max_frame_rate(FrameRate::from_hz(30.0))
                .rate_control_mode(RateControlMode::Bitrate),
        ),
        (
            "+usage nonrealtime",
            base().usage_type(UsageType::CameraVideoNonRealTime),
        ),
        ("+complexity high", base().complexity(Complexity::High)),
        ("+threads 0", base().num_threads(0)),
        ("+threads 8", base().num_threads(8)),
        ("+skip false", base().skip_frames(false)),
        ("+vui bt709", base().vui(VuiConfig::bt709())),
    ];
    let yuv = YUVBuffer::new(1920, 1080);
    for (name, cfg) in configs {
        let api = OpenH264API::from_blob_path(dll).expect("load dll");
        let r = Encoder::with_api_config(api, cfg)
            .map_err(|e| e.to_string())
            .and_then(|mut e| {
                e.encode(&yuv)
                    .map(|b| b.to_vec().len())
                    .map_err(|e| e.to_string())
            });
        println!("{name:<22} {r:?}");
    }
}
