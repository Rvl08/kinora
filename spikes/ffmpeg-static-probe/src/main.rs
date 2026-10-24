//! P0-05b probe: proves that Kinora's static release FFmpeg links into a Rust
//! executable (no FFmpeg DLL, static CRT) and works. Throwaway, see
//! spikes/README.md; results in docs/spikes/P0-05b-report.md.
//!
//! 1. Prints the configuration and licence compiled into the libraries.
//! 2. Looks up every encoder and decoder named in release-components.json.
//! 3. Encodes a generated clip with each external library (SVT-AV1, libvpx,
//!    Opus, LAME) and several native encoders, writes it with the release
//!    muxers, then demuxes and decodes it again (dav1d, native VP9, ...) and
//!    compares frame counts and pixels or audio level.
//! 4. Runs a small filter graph and a software scaler.
//!
//! Usage: ffmpeg-static-probe [<work dir>] [--json <report.json>]
//! Exit code 0 only when every check passes.

use std::{
    ffi::CStr,
    fmt::Display,
    path::{Path, PathBuf},
};

use ffmpeg::{
    ChannelLayout, Dictionary, Packet, Rational, codec, decoder, encoder, filter, format, frame,
    media, software::resampling, software::scaling,
};
use ffmpeg_next as ffmpeg;
use serde_json::{Value, json};

const COMPONENTS: &str = include_str!("../../../scripts/ffmpeg/release-components.json");
const W: u32 = 320;
const H: u32 = 240;
const FPS: i32 = 24;
const FRAMES: usize = 24;
const RATE: i32 = 48_000;
const AUDIO_SECONDS: usize = 1;
const TONE_HZ: f32 = 440.0;
const AMPLITUDE: f32 = 0.5;

fn err<E: Display>(context: &'static str) -> impl Fn(E) -> String {
    move |e| format!("{context}: {e}")
}

fn cstr(ptr: *const std::os::raw::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: FFmpeg returns static NUL-terminated strings.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

// ------------------------------------------------------------------ synthetic media

/// Expected luma of the generated clip: a horizontal gradient that scrolls
/// two pixels per frame, plus a 32x32 white square moving diagonally.
fn luma(x: u32, y: u32, t: usize) -> u8 {
    let sx = 16 + 6 * t as u32;
    let sy = 16 + 4 * t as u32;
    if (sx..sx + 32).contains(&x) && (sy..sy + 32).contains(&y) {
        return 235;
    }
    (16 + ((x + 2 * t as u32) * 200 / (W + 2 * FRAMES as u32))) as u8
}

fn depth16(pixel: format::Pixel) -> bool {
    matches!(
        pixel,
        format::Pixel::YUV422P10LE | format::Pixel::YUV420P10LE
    )
}

fn synth_frame(pixel: format::Pixel, t: usize) -> frame::Video {
    let mut f = frame::Video::new(pixel, W, H);
    let wide = depth16(pixel);
    for plane in 0..f.planes() {
        let (pw, ph) = (
            f.plane_width(plane) as usize,
            f.plane_height(plane) as usize,
        );
        let stride = f.stride(plane);
        let data = f.data_mut(plane);
        for y in 0..ph {
            for x in 0..pw {
                let v = if plane == 0 {
                    luma(x as u32, y as u32, t)
                } else {
                    128
                };
                if wide {
                    let v = u16::from(v) << 2;
                    data[y * stride + 2 * x..y * stride + 2 * x + 2]
                        .copy_from_slice(&v.to_le_bytes());
                } else {
                    data[y * stride + x] = v;
                }
            }
        }
    }
    f.set_pts(Some(t as i64));
    f
}

/// Mean absolute luma error (8-bit scale) of a decoded frame against frame `t`.
fn luma_error(f: &frame::Video, t: usize) -> f64 {
    let wide = depth16(f.format());
    let stride = f.stride(0);
    let data = f.data(0);
    let mut sum = 0f64;
    for y in 0..H {
        for x in 0..W {
            let at = y as usize * stride;
            let got = if wide {
                f64::from(
                    u16::from_le_bytes([data[at + 2 * x as usize], data[at + 2 * x as usize + 1]])
                        >> 2,
                )
            } else {
                f64::from(data[at + x as usize])
            };
            // The generator writes the same code values whatever the range tag
            // (yuvj420p for MJPEG), so no range conversion is needed.
            let want = f64::from(luma(x, y, t));
            sum += (got - want).abs();
        }
    }
    sum / f64::from(W * H)
}

// ------------------------------------------------------------------ video round trip

struct VideoCase {
    encoder: &'static str,
    decoder: &'static str,
    file: &'static str,
    pixel: format::Pixel,
    options: &'static [(&'static str, &'static str)],
    max_error: f64,
}

fn encode_video(case: &VideoCase, path: &Path) -> Result<usize, String> {
    let codec = encoder::find_by_name(case.encoder).ok_or("encoder not registered")?;
    let mut octx = format::output(path).map_err(err("output"))?;
    let global = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);
    let mut ctx = codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()
        .map_err(err("encoder context"))?;
    ctx.set_width(W);
    ctx.set_height(H);
    ctx.set_format(case.pixel);
    ctx.set_time_base(Rational(1, FPS));
    ctx.set_frame_rate(Some(Rational(FPS, 1)));
    if global {
        ctx.set_flags(codec::Flags::GLOBAL_HEADER);
    }
    let mut options = Dictionary::new();
    for (k, v) in case.options {
        options.set(k, v);
    }
    let mut enc = ctx.open_with(options).map_err(err("open encoder"))?;
    let mut ost = octx.add_stream(codec).map_err(err("add stream"))?;
    ost.set_parameters(&enc);
    ost.set_time_base(Rational(1, FPS));
    octx.write_header().map_err(err("write header"))?;
    let ost_tb = octx.stream(0).ok_or("no stream")?.time_base();

    let mut packets = 0;
    let mut drain = |enc: &mut encoder::video::Encoder,
                     octx: &mut format::context::Output|
     -> Result<(), String> {
        let mut pkt = Packet::empty();
        while enc.receive_packet(&mut pkt).is_ok() {
            pkt.set_stream(0);
            pkt.rescale_ts(Rational(1, FPS), ost_tb);
            pkt.write_interleaved(octx).map_err(err("mux"))?;
            packets += 1;
        }
        Ok(())
    };
    for t in 0..FRAMES {
        enc.send_frame(&synth_frame(case.pixel, t))
            .map_err(err("send frame"))?;
        drain(&mut enc, &mut octx)?;
    }
    enc.send_eof().map_err(err("send eof"))?;
    drain(&mut enc, &mut octx)?;
    octx.write_trailer().map_err(err("write trailer"))?;
    Ok(packets)
}

fn decode_video(case: &VideoCase, path: &Path) -> Result<(usize, f64, String), String> {
    let mut ictx = format::input(path).map_err(err("input"))?;
    let demuxer = ictx.format().name().to_owned();
    let stream = ictx
        .streams()
        .best(media::Type::Video)
        .ok_or("no video stream")?;
    let index = stream.index();
    let codec = decoder::find_by_name(case.decoder).ok_or("decoder not registered")?;
    let mut dec = codec::context::Context::from_parameters(stream.parameters())
        .map_err(err("decoder context"))?
        .decoder()
        .open_as(codec)
        .map_err(err("open decoder"))?
        .video()
        .map_err(err("video decoder"))?;
    let mut frames = 0usize;
    let mut worst = 0f64;
    let mut decoded = frame::Video::empty();
    let mut take = |dec: &mut decoder::Video, frames: &mut usize| {
        while dec.receive_frame(&mut decoded).is_ok() {
            worst = worst.max(luma_error(&decoded, *frames));
            *frames += 1;
        }
    };
    for (s, packet) in ictx.packets() {
        if s.index() == index {
            dec.send_packet(&packet).map_err(err("send packet"))?;
            take(&mut dec, &mut frames);
        }
    }
    dec.send_eof().map_err(err("decoder eof"))?;
    take(&mut dec, &mut frames);
    Ok((frames, worst, demuxer))
}

fn video_round_trip(case: &VideoCase, dir: &Path) -> Value {
    let path = dir.join(case.file);
    let result = encode_video(case, &path).and_then(|packets| {
        let (frames, worst, demuxer) = decode_video(case, &path)?;
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let ok = frames == FRAMES && worst <= case.max_error;
        Ok(json!({
            "ok": ok, "packets": packets, "frames": frames, "worstLumaError": (worst * 100.0).round() / 100.0,
            "bytes": bytes, "demuxer": demuxer,
        }))
    });
    let mut value = result.unwrap_or_else(|e| json!({ "ok": false, "error": e }));
    value["encoder"] = json!(case.encoder);
    value["decoder"] = json!(case.decoder);
    value["file"] = json!(case.file);
    value
}

// ------------------------------------------------------------------ audio round trip

struct AudioCase {
    encoder: &'static str,
    decoder: &'static str,
    file: &'static str,
}

fn sample(i: usize) -> f32 {
    AMPLITUDE * (2.0 * std::f32::consts::PI * TONE_HZ * i as f32 / RATE as f32).sin()
}

/// Fills `n` stereo samples starting at `start`, in whatever format the encoder takes.
fn synth_audio(fmt: format::Sample, start: usize, n: usize) -> frame::Audio {
    use format::sample::Type;
    let mut f = frame::Audio::new(fmt, n, ChannelLayout::default(2));
    f.set_rate(RATE as u32);
    f.set_pts(Some(start as i64));
    let values: Vec<f32> = (start..start + n).map(sample).collect();
    match fmt {
        format::Sample::F32(Type::Planar) => {
            for c in 0..2 {
                f.plane_mut::<f32>(c)[..n].copy_from_slice(&values);
            }
        }
        format::Sample::F32(Type::Packed) => {
            let data = f.data_mut(0);
            for (i, v) in values.iter().enumerate() {
                for c in 0..2 {
                    data[(2 * i + c) * 4..(2 * i + c) * 4 + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
        format::Sample::I16(kind) => {
            let ints: Vec<i16> = values.iter().map(|v| (v * 32767.0) as i16).collect();
            fill_ints(&mut f, kind, &ints, 2);
        }
        format::Sample::I32(kind) => {
            let ints: Vec<i32> = values
                .iter()
                .map(|v| (v * 2_147_483_647.0) as i32)
                .collect();
            fill_ints(&mut f, kind, &ints, 4);
        }
        other => panic!("probe cannot generate {other:?}"),
    }
    f
}

trait LeBytes: Copy {
    fn le(self) -> Vec<u8>;
}
impl LeBytes for i16 {
    fn le(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl LeBytes for i32 {
    fn le(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

fn fill_ints<T: LeBytes>(
    f: &mut frame::Audio,
    kind: format::sample::Type,
    ints: &[T],
    size: usize,
) {
    if kind == format::sample::Type::Planar {
        let len = ints.len() * size;
        for c in 0..2 {
            // ffmpeg-next's data_mut(c) sizes plane c by linesize[c], but for
            // planar audio FFmpeg sets only linesize[0] (all planes share it),
            // so plane 1 came back empty (CI run 40063055337, libmp3lame s32p).
            // SAFETY: the frame was allocated by av_frame_get_buffer for
            // ints.len() samples of `size` bytes in each of its 2 planes, so
            // data[c] points to at least `len` writable bytes.
            let data = unsafe {
                let plane = (*f.as_mut_ptr()).data[c];
                assert!(!plane.is_null(), "audio plane {c} not allocated");
                std::slice::from_raw_parts_mut(plane, len)
            };
            for (i, v) in ints.iter().enumerate() {
                data[i * size..(i + 1) * size].copy_from_slice(&v.le());
            }
        }
    } else {
        let data = f.data_mut(0);
        for (i, v) in ints.iter().enumerate() {
            for c in 0..2 {
                data[(2 * i + c) * size..(2 * i + c + 1) * size].copy_from_slice(&v.le());
            }
        }
    }
}

fn encode_audio(case: &AudioCase, path: &Path) -> Result<usize, String> {
    let codec = encoder::find_by_name(case.encoder).ok_or("encoder not registered")?;
    let fmt = codec
        .audio()
        .map_err(err("audio codec"))?
        .formats()
        .and_then(|mut f| f.next())
        .ok_or("encoder lists no sample format")?;
    let mut octx = format::output(path).map_err(err("output"))?;
    let global = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);
    let mut ctx = codec::context::Context::new_with_codec(codec)
        .encoder()
        .audio()
        .map_err(err("encoder context"))?;
    ctx.set_rate(RATE);
    ctx.set_channel_layout(ChannelLayout::default(2));
    ctx.set_format(fmt);
    ctx.set_bit_rate(128_000);
    ctx.set_time_base(Rational(1, RATE));
    if global {
        ctx.set_flags(codec::Flags::GLOBAL_HEADER);
    }
    let mut enc = ctx
        .open_with(Dictionary::new())
        .map_err(err("open encoder"))?;
    let mut ost = octx.add_stream(codec).map_err(err("add stream"))?;
    ost.set_parameters(&enc);
    ost.set_time_base(Rational(1, RATE));
    octx.write_header().map_err(err("write header"))?;
    let ost_tb = octx.stream(0).ok_or("no stream")?.time_base();

    let total = RATE as usize * AUDIO_SECONDS;
    let chunk = if enc.frame_size() > 0 {
        enc.frame_size() as usize
    } else {
        1024
    };
    let mut packets = 0;
    let mut drain = |enc: &mut encoder::audio::Encoder,
                     octx: &mut format::context::Output|
     -> Result<(), String> {
        let mut pkt = Packet::empty();
        while enc.receive_packet(&mut pkt).is_ok() {
            pkt.set_stream(0);
            pkt.rescale_ts(Rational(1, RATE), ost_tb);
            // Describe the packet before muxing: the muxer may fail without
            // logging anything (FLAC, CI run 40065912310).
            let what = format!(
                "packet {packets}: size {} pts {:?} dts {:?} duration {} side data {}, stream extradata {} bytes",
                pkt.size(),
                pkt.pts(),
                pkt.dts(),
                pkt.duration(),
                pkt.side_data().count(),
                // SAFETY: stream 0 exists (added above); codecpar is owned by it.
                unsafe { (*octx.stream(0).ok_or("no stream")?.parameters().as_ptr()).extradata_size },
            );
            pkt.write_interleaved(octx)
                .map_err(|e| format!("mux {what}: {e}"))?;
            packets += 1;
        }
        Ok(())
    };
    let mut pos = 0;
    while pos < total {
        // Encoders with a fixed frame size accept a short frame only at the end.
        let n = chunk.min(total - pos);
        enc.send_frame(&synth_audio(fmt, pos, n))
            .map_err(err("send frame"))?;
        drain(&mut enc, &mut octx)?;
        pos += n;
    }
    enc.send_eof().map_err(err("send eof"))?;
    drain(&mut enc, &mut octx)?;
    octx.write_trailer().map_err(err("write trailer"))?;
    Ok(packets)
}

/// Decodes, converts to packed f32 through swresample, and measures length and level.
fn decode_audio(case: &AudioCase, path: &Path) -> Result<(usize, f64, String), String> {
    let mut ictx = format::input(path).map_err(err("input"))?;
    let demuxer = ictx.format().name().to_owned();
    let stream = ictx
        .streams()
        .best(media::Type::Audio)
        .ok_or("no audio stream")?;
    let index = stream.index();
    let codec = decoder::find_by_name(case.decoder).ok_or("decoder not registered")?;
    let mut dec = codec::context::Context::from_parameters(stream.parameters())
        .map_err(err("decoder context"))?
        .decoder()
        .open_as(codec)
        .map_err(err("open decoder"))?
        .audio()
        .map_err(err("audio decoder"))?;
    let mut resampler: Option<resampling::Context> = None;
    let mut samples = 0usize;
    let mut energy = 0f64;
    let mut decoded = frame::Audio::empty();
    let mut take = |dec: &mut decoder::Audio| -> Result<(), String> {
        while dec.receive_frame(&mut decoded).is_ok() {
            // WAV without a channel mask decodes to an "unspecified" layout,
            // which swr_convert_frame treats as different from the default
            // stereo layout and rejects with "Input changed" (CI run
            // 40065912310, pcm_s16le). Name it the default layout for its
            // channel count before building or running the resampler.
            if decoded.channel_layout().is_empty() {
                let channels = i32::from(decoded.channels());
                decoded.set_channel_layout(ChannelLayout::default(channels));
            }
            if resampler.is_none() {
                resampler = Some(
                    resampling::Context::get(
                        decoded.format(),
                        decoded.channel_layout(),
                        decoded.rate(),
                        format::Sample::F32(format::sample::Type::Packed),
                        ChannelLayout::default(2),
                        RATE as u32,
                    )
                    .map_err(err("swresample"))?,
                );
            }
            let rs = resampler.as_mut().ok_or("no resampler")?;
            let mut out = frame::Audio::new(
                format::Sample::F32(format::sample::Type::Packed),
                decoded.samples() + 256,
                ChannelLayout::default(2),
            );
            rs.run(&decoded, &mut out).map_err(err("resample"))?;
            let n = out.samples();
            let data = out.data(0);
            for i in 0..n * 2 {
                let v = f32::from_le_bytes([
                    data[4 * i],
                    data[4 * i + 1],
                    data[4 * i + 2],
                    data[4 * i + 3],
                ]);
                energy += f64::from(v) * f64::from(v);
            }
            samples += n;
        }
        Ok(())
    };
    for (s, packet) in ictx.packets() {
        if s.index() == index {
            dec.send_packet(&packet).map_err(err("send packet"))?;
            take(&mut dec)?;
        }
    }
    dec.send_eof().map_err(err("decoder eof"))?;
    take(&mut dec)?;
    let rms = if samples > 0 {
        (energy / (2 * samples) as f64).sqrt()
    } else {
        0.0
    };
    Ok((samples, rms, demuxer))
}

fn audio_round_trip(case: &AudioCase, dir: &Path) -> Value {
    let path = dir.join(case.file);
    let total = RATE as usize * AUDIO_SECONDS;
    let expected_rms = f64::from(AMPLITUDE) / 2f64.sqrt();
    let result = encode_audio(case, &path).and_then(|packets| {
        let (samples, rms, demuxer) = decode_audio(case, &path)?;
        let length_ok = samples as f64 >= 0.95 * total as f64 && samples as f64 <= 1.10 * total as f64;
        let level_ok = (rms - expected_rms).abs() <= 0.1 * expected_rms;
        Ok(json!({
            "ok": length_ok && level_ok, "packets": packets, "samples": samples,
            "rms": (rms * 1000.0).round() / 1000.0, "expectedRms": (expected_rms * 1000.0).round() / 1000.0,
            "demuxer": demuxer,
        }))
    });
    let mut value = result.unwrap_or_else(|e| json!({ "ok": false, "error": e }));
    value["encoder"] = json!(case.encoder);
    value["decoder"] = json!(case.decoder);
    value["file"] = json!(case.file);
    value
}

// ------------------------------------------------------------------ filters and scaler

fn filter_and_scale() -> Result<Value, String> {
    let source = synth_frame(format::Pixel::YUV420P, 0);

    // Scale first: the buffer source below takes the frame's references and
    // leaves it blank (av_buffersrc_add_frame), and swscale then rejected the
    // blank frame with "Input changed" (CI run 40065912310).
    let mut scaler = scaling::Context::get(
        format::Pixel::YUV420P,
        W,
        H,
        format::Pixel::RGB24,
        W / 2,
        H / 2,
        scaling::Flags::BILINEAR,
    )
    .map_err(err("swscale"))?;
    let mut rgb = frame::Video::empty();
    scaler.run(&source, &mut rgb).map_err(err("scale"))?;

    let mut graph = filter::Graph::new();
    let args = format!(
        "video_size={W}x{H}:pix_fmt={}:time_base=1/{FPS}:pixel_aspect=1/1",
        ffmpeg::ffi::AVPixelFormat::AV_PIX_FMT_YUV420P as i32
    );
    graph
        .add(
            &filter::find("buffer").ok_or("no buffer filter")?,
            "in",
            &args,
        )
        .map_err(err("buffer"))?;
    graph
        .add(
            &filter::find("buffersink").ok_or("no buffersink filter")?,
            "out",
            "",
        )
        .map_err(err("buffersink"))?;
    graph
        .output("in", 0)
        .and_then(|p| p.input("out", 0))
        .and_then(|p| p.parse("scale=160:120,hflip,pad=176:128:8:4"))
        .map_err(err("graph"))?;
    graph.validate().map_err(err("validate"))?;
    graph
        .get("in")
        .ok_or("no in")?
        .source()
        .add(&source)
        .map_err(err("push"))?;
    let mut filtered = frame::Video::empty();
    graph
        .get("out")
        .ok_or("no out")?
        .sink()
        .frame(&mut filtered)
        .map_err(err("pull"))?;

    let ok = (filtered.width(), filtered.height()) == (176, 128)
        && (rgb.width(), rgb.height()) == (W / 2, H / 2);
    Ok(json!({
        "ok": ok,
        "graph": "scale=160:120,hflip,pad=176:128:8:4",
        "graphOutput": format!("{}x{}", filtered.width(), filtered.height()),
        "swscaleOutput": format!("{}x{} {:?}", rgb.width(), rgb.height(), rgb.format()),
    }))
}

// ------------------------------------------------------------------ main

fn names(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        Value::Object(map) => map.values().flat_map(names).collect(),
        _ => Vec::new(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json_path = args
        .iter()
        .position(|a| a == "--json")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let work = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ffmpeg-static-probe"));
    std::fs::create_dir_all(&work).expect("work dir");

    ffmpeg::init().expect("ffmpeg init");
    // SAFETY: plain getters returning static strings.
    let (version, configuration, licence) = unsafe {
        (
            cstr(ffmpeg::ffi::av_version_info()),
            cstr(ffmpeg::ffi::avcodec_configuration()),
            cstr(ffmpeg::ffi::avcodec_license()),
        )
    };
    println!("FFmpeg {version}");
    println!("avcodec_license(): {licence}");
    println!("avcodec_configuration(): {configuration}");

    // Every listed encoder and decoder must be registered.
    let spec: Value = serde_json::from_str(COMPONENTS).expect("release-components.json");
    let mut lookup_failures = Vec::new();
    let decoders = names(&spec["decoders"]);
    let encoders = names(&spec["encoders"]);
    for name in &decoders {
        let found = decoder::find_by_name(name);
        println!(
            "decoder {name:<14} {}",
            if found.is_some() { "ok" } else { "MISSING" }
        );
        if found.is_none() {
            lookup_failures.push(format!("decoder {name}"));
        }
    }
    for name in &encoders {
        let found = encoder::find_by_name(name);
        println!(
            "encoder {name:<14} {}",
            if found.is_some() { "ok" } else { "MISSING" }
        );
        if found.is_none() {
            lookup_failures.push(format!("encoder {name}"));
        }
    }
    let mut registered = 0;
    let mut opaque: *mut std::ffi::c_void = std::ptr::null_mut();
    // SAFETY: av_codec_iterate walks a static list with an opaque cursor.
    while !unsafe { ffmpeg::ffi::av_codec_iterate(&mut opaque) }.is_null() {
        registered += 1;
    }
    let mut hwdevices = Vec::new();
    let mut kind = ffmpeg::ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_NONE;
    loop {
        // SAFETY: iterating the compiled-in hardware device types.
        kind = unsafe { ffmpeg::ffi::av_hwdevice_iterate_types(kind) };
        if kind == ffmpeg::ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_NONE {
            break;
        }
        hwdevices.push(cstr(unsafe {
            ffmpeg::ffi::av_hwdevice_get_type_name(kind)
        }));
    }
    println!(
        "{registered} codecs registered; hardware device types: {}",
        hwdevices.join(", ")
    );

    let video = [
        VideoCase {
            encoder: "libsvtav1",
            decoder: "libdav1d",
            file: "svtav1.mkv",
            pixel: format::Pixel::YUV420P,
            options: &[("preset", "12")],
            max_error: 12.0,
        },
        VideoCase {
            encoder: "libvpx-vp9",
            decoder: "vp9",
            file: "vp9.webm",
            pixel: format::Pixel::YUV420P,
            options: &[("deadline", "realtime"), ("cpu-used", "8")],
            max_error: 12.0,
        },
        VideoCase {
            encoder: "prores_ks",
            decoder: "prores",
            file: "prores.mov",
            pixel: format::Pixel::YUV422P10LE,
            options: &[("profile", "standard")],
            max_error: 3.0,
        },
        VideoCase {
            encoder: "mjpeg",
            decoder: "mjpeg",
            file: "mjpeg.mp4",
            pixel: format::Pixel::YUVJ420P,
            options: &[],
            max_error: 8.0,
        },
    ];
    let audio = [
        AudioCase {
            encoder: "libopus",
            decoder: "opus",
            file: "opus.opus",
        },
        AudioCase {
            encoder: "libmp3lame",
            decoder: "mp3float",
            file: "lame.mp3",
        },
        AudioCase {
            encoder: "aac",
            decoder: "aac",
            file: "aac.m4a",
        },
        AudioCase {
            encoder: "flac",
            decoder: "flac",
            file: "flac.flac",
        },
        AudioCase {
            encoder: "pcm_s16le",
            decoder: "pcm_s16le",
            file: "pcm.wav",
        },
    ];

    let mut round_trips = Vec::new();
    for case in &video {
        let result = video_round_trip(case, &work);
        println!("video {}", result);
        round_trips.push(result);
    }
    for case in &audio {
        let result = audio_round_trip(case, &work);
        println!("audio {}", result);
        round_trips.push(result);
    }
    let graph = filter_and_scale().unwrap_or_else(|e| json!({ "ok": false, "error": e }));
    println!("filters {graph}");

    let failed_trips = round_trips
        .iter()
        .filter(|r| r["ok"] != json!(true))
        .count();
    let ok = lookup_failures.is_empty()
        && failed_trips == 0
        && graph["ok"] == json!(true)
        && licence.contains("LGPL");
    let report = json!({
        "ok": ok,
        "version": version,
        "licence": licence,
        "configuration": configuration,
        "registeredCodecs": registered,
        "listedDecoders": decoders.len(),
        "listedEncoders": encoders.len(),
        "lookupFailures": lookup_failures,
        "hwdevices": hwdevices,
        "roundTrips": round_trips,
        "filters": graph,
        "exe": std::env::current_exe().ok().and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len()),
    });
    if let Some(path) = json_path {
        std::fs::write(&path, serde_json::to_string_pretty(&report).expect("json"))
            .expect("write report");
    }
    println!(
        "probe: {} ({} lookups failed, {}/{} round trips failed)",
        if ok { "OK" } else { "FAILED" },
        lookup_failures.len(),
        failed_trips,
        round_trips.len()
    );
    std::process::exit(if ok { 0 } else { 1 });
}
