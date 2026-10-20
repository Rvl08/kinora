//! Spike S2: media decode/encode paths. Throwaway.
//!
//! `cargo run --release -- bench [report.json]` runs every measurement and
//! writes one JSON report (default: tests/media/s2-out/report.json).

mod audio;
mod composite;
mod hw;
mod pipeline;
mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

use ffmpeg_next as ffmpeg;
use serde_json::{Value, json};

use hw::Device;
use pipeline::{Job, Mode, transcode};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}
fn p(rel: &str) -> String {
    root()
        .join(rel)
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_owned()
}

const ALL_ENCODERS: &[&str] = &[
    "h264_nvenc",
    "hevc_nvenc",
    "av1_nvenc",
    "h264_qsv",
    "hevc_qsv",
    "av1_qsv",
    "h264_amf",
    "hevc_amf",
    "av1_amf",
    "libopenh264",
    "libsvtav1",
    "libvpx-vp9",
    "prores_ks",
    "dnxhd",
];

fn options(encoder: &str, uhd: bool) -> Vec<(&'static str, &'static str)> {
    match encoder {
        e if e.ends_with("_nvenc") => vec![("preset", "p5"), ("tune", "hq"), ("rc", "vbr")],
        e if e.ends_with("_qsv") => vec![("preset", "medium")],
        e if e.ends_with("_amf") => vec![("quality", "balanced")],
        "libsvtav1" => vec![("preset", if uhd { "10" } else { "8" })],
        "libvpx-vp9" => vec![
            ("deadline", "good"),
            ("cpu-used", "5"),
            ("row-mt", "1"),
            ("tile-columns", "2"),
        ],
        "libopenh264" => vec![("rc_mode", "bitrate"), ("allow_skip_frames", "0")],
        "prores_ks" => vec![("profile", "standard")],
        "dnxhd" => vec![("profile", "dnxhr_hq")],
        _ => vec![],
    }
}

fn ext(encoder: &str) -> &'static str {
    match encoder {
        "libvpx-vp9" => "webm",
        "prores_ks" | "dnxhd" => "mov",
        _ => "mp4",
    }
}

fn job<'a>(
    input: &'a str,
    output: &'a str,
    encoder: &'a str,
    mode: Mode,
    device: Device,
    bitrate: i64,
    max_frames: usize,
    uhd: bool,
) -> Job<'a> {
    Job {
        input,
        output,
        encoder,
        mode,
        device,
        bitrate,
        gop: 60,
        b_frames: None,
        max_frames: Some(max_frames),
        scale: None,
        options: options(encoder, uhd),
        keep_10bit: true,
        audio: None,
    }
}

fn log(v: &Value) {
    println!("{}", serde_json::to_string(v).unwrap());
}

fn ffprobe(path: &str) -> Value {
    let out = Command::new(p("vendor/ffmpeg-lgpl/bin/ffprobe.exe"))
        .args(["-v", "error", "-show_entries", "stream=index,codec_name,profile,pix_fmt,width,height,color_space,color_range,color_primaries,color_transfer,duration,start_time,nb_frames,sample_rate,channels:format=duration,size", "-of", "json", path])
        .output()
        .expect("ffprobe");
    serde_json::from_slice(&out.stdout).unwrap_or(Value::Null)
}

fn main() {
    ffmpeg::init().expect("ffmpeg init");
    unsafe { ffmpeg::ffi::av_log_set_level(ffmpeg::ffi::AV_LOG_ERROR) };
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("openh264-debug") {
        tools::openh264_debug(&p("vendor/openh264/openh264-2.6.0-win64.dll"));
        return;
    }
    if args.get(1).map(String::as_str) != Some("bench") {
        eprintln!("usage: s2-media bench [report.json]");
        std::process::exit(2);
    }
    let out_dir = p("tests/media/s2-out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let report_path = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| format!("{out_dir}\\report.json"));
    // Optional comma-separated section filter, e.g. "quality,modes".
    let sections: Option<Vec<String>> = args
        .get(3)
        .map(|s| s.split(',').map(str::to_owned).collect());
    let want = |name: &str| {
        sections
            .as_ref()
            .is_none_or(|v| v.iter().any(|x| x == name))
    };
    let o = |name: &str| format!("{out_dir}\\{name}");
    let ffmpeg_exe = p("vendor/ffmpeg-lgpl/bin/ffmpeg.exe");
    let mut report = json!({ "spike": "S2 media", "started": now() });

    // 1. Encoder probe (start-up capability detection).
    let (probes, total_ms) = tools::probe_encoders(ALL_ENCODERS);
    let opens: Vec<String> = probes
        .iter()
        .filter(|p| p.opens)
        .map(|p| p.encoder.clone())
        .collect();
    report["probe"] = json!({ "totalMs": total_ms, "encoders": probes });
    log(&report["probe"]);
    let usable = |e: &str| opens.iter().any(|x| x == e);

    let src_1080 = p("tests/media/s1/1080p30-h264.mp4");
    let src_4k8 = p("tests/media/s2/2160p60-h264.mp4");
    let src_4k10 = p("tests/media/s1/2160p60-hevc10.mp4");

    if want("speed1080p") {
        // 2. Encoder matrix: speed at 1080p30 (H.264 source, D3D11VA decode + download).
        let mut matrix = Vec::new();
        for enc in [
            "h264_nvenc",
            "hevc_nvenc",
            "av1_nvenc",
            "h264_qsv",
            "hevc_qsv",
            "av1_qsv",
            "h264_amf",
            "hevc_amf",
            "av1_amf",
            "libopenh264",
            "libsvtav1",
            "libvpx-vp9",
            "prores_ks",
            "dnxhd",
        ] {
            if !usable(enc) {
                matrix.push(json!({ "encoder": enc, "skipped": "does not open on this machine" }));
                continue;
            }
            let out = o(&format!("speed-1080p-{enc}.{}", ext(enc)));
            let r = transcode(
                &job(
                    &src_1080,
                    &out,
                    enc,
                    Mode::HwDownload,
                    Device::D3d11va,
                    8_000_000,
                    600,
                    false,
                ),
                &mut None,
            );
            let v = serde_json::to_value(&r).unwrap();
            log(&v);
            matrix.push(v);
        }
        report["speed1080p"] = json!(matrix);
    }

    if want("speed4k") {
        // 3. 4K: 8-bit H.264 source and 10-bit HEVC source (10-bit kept where the encoder can).
        let mut uhd = Vec::new();
        for (src, tag) in [(&src_4k8, "h264-8bit"), (&src_4k10, "hevc-10bit")] {
            for enc in ["h264_nvenc", "hevc_nvenc", "av1_nvenc", "libsvtav1"] {
                if !usable(enc) {
                    continue;
                }
                let out = o(&format!("speed-4k-{tag}-{enc}.mp4"));
                let r = transcode(
                    &job(
                        src,
                        &out,
                        enc,
                        Mode::HwDownload,
                        Device::D3d11va,
                        40_000_000,
                        600,
                        true,
                    ),
                    &mut None,
                );
                let mut v = serde_json::to_value(&r).unwrap();
                v["source"] = json!(tag);
                v["outputStream"] = ffprobe(&out)["streams"][0].clone();
                log(&v);
                uhd.push(v);
            }
        }
        report["speed4k"] = json!(uhd);
    }

    if want("modes") {
        // 4. Pipeline modes with NVENC H.264.
        let mut modes = Vec::new();
        for (src, tag, uhd_src, frames) in [
            (&src_1080, "1080p30", false, 900),
            (&src_4k8, "2160p60-8bit", true, 600),
        ] {
            for (mode, dev) in [
                (Mode::Sw, Device::D3d11va),
                (Mode::HwDownload, Device::D3d11va),
                (Mode::HwDownload, Device::Cuda),
                (Mode::ZeroCopy, Device::D3d11va),
                (Mode::ZeroCopy, Device::Cuda),
                (Mode::Composite, Device::D3d11va),
            ] {
                let out = o(&format!("mode-{tag}-{mode:?}-{dev:?}.mp4"));
                let br = if uhd_src { 40_000_000 } else { 8_000_000 };
                let mut comp = None;
                let r = transcode(
                    &job(src, &out, "h264_nvenc", mode, dev, br, frames, uhd_src),
                    &mut comp,
                );
                let mut v = serde_json::to_value(&r).unwrap();
                v["source"] = json!(tag);
                v["device"] = json!(format!("{dev:?}"));
                if let Some(c) = comp {
                    v["compositeGpuWaitS"] = json!(c.gpu_wait_s);
                    v["compositeFrames"] = json!(c.frames);
                }
                log(&v);
                modes.push(v);
            }
        }
        report["modes"] = json!(modes);
    }

    if want("quality") {
        // 5. Quality at equal bitrate (6 Mbps, 1080p30), lossless references.
        let mut quality = Vec::new();
        for (content, reference) in [
            ("graphics", p("tests/media/s2/graphics-1080p30-ffv1.mkv")),
            ("natural", p("tests/media/s2/natural-1080p30-ffv1.mkv")),
        ] {
            let mut variants: Vec<(String, &str, Vec<(&'static str, &'static str)>)> = Vec::new();
            for enc in [
                "h264_nvenc",
                "hevc_nvenc",
                "av1_nvenc",
                "h264_qsv",
                "h264_amf",
                "libopenh264",
                "libsvtav1",
                "libvpx-vp9",
            ] {
                if usable(enc) {
                    variants.push((enc.to_owned(), enc, options(enc, false)));
                }
            }
            if usable("h264_nvenc") {
                variants.push((
                    "h264_nvenc-p7".into(),
                    "h264_nvenc",
                    vec![
                        ("preset", "p7"),
                        ("tune", "hq"),
                        ("rc", "vbr"),
                        ("multipass", "fullres"),
                    ],
                ));
            }
            for (label, enc, opts) in variants {
                let out = o(&format!("q-{content}-{label}.{}", ext(enc)));
                let mut j = job(
                    &reference,
                    &out,
                    enc,
                    Mode::Sw,
                    Device::D3d11va,
                    6_000_000,
                    300,
                    false,
                );
                j.options = opts;
                let r = transcode(&j, &mut None);
                let mut v = serde_json::to_value(&r).unwrap();
                v["content"] = json!(content);
                v["label"] = json!(label);
                if r.error.is_none() {
                    v["quality"] = tools::quality(
                        &ffmpeg_exe,
                        &out,
                        &reference,
                        &o(&format!("vmaf-{content}-{label}.json")),
                    )
                    .unwrap_or_else(|e| json!({ "error": e }));
                }
                log(&v);
                quality.push(v);
            }
            // OpenH264 through Cisco's DLL.
            let raw = o(&format!("q-{content}-openh264-cisco.h264"));
            let mp4 = o(&format!("q-{content}-openh264-cisco.mp4"));
            let mut v = tools::openh264_cisco(
                &p("vendor/openh264/openh264-2.6.0-win64.dll"),
                &reference,
                &raw,
                6_000_000,
                300,
            )
            .unwrap_or_else(|e| json!({ "error": e }));
            v["content"] = json!(content);
            v["label"] = json!("openh264-cisco-dll");
            if v["error"].is_null() && tools::remux_h264(&ffmpeg_exe, &raw, 30.0, &mp4).is_ok() {
                v["quality"] = tools::quality(
                    &ffmpeg_exe,
                    &mp4,
                    &reference,
                    &o(&format!("vmaf-{content}-openh264-cisco.json")),
                )
                .unwrap_or_else(|e| json!({ "error": e }));
            }
            log(&v);
            quality.push(v);
        }
        report["quality"] = json!(quality);
    }

    if want("proxies") {
        // 6. Proxies: 4K sources -> 1080p H.264, GOP 10, no B-frames.
        let mut proxies = Vec::new();
        for (src, tag, mode) in [
            (&src_4k10, "hevc10-4k60", Mode::HwDownload),
            (
                &p("tests/media/s1/2160p30-prores.mov"),
                "prores-4k30",
                Mode::Sw,
            ),
            (&src_4k8, "h264-4k60", Mode::HwDownload),
        ] {
            let out = o(&format!("proxy-{tag}.mp4"));
            let mut j = job(
                src,
                &out,
                "h264_nvenc",
                mode,
                Device::D3d11va,
                8_000_000,
                600,
                false,
            );
            j.gop = 10;
            j.b_frames = Some(0);
            j.scale = Some((1920, 1080));
            j.keep_10bit = false;
            j.max_frames = None;
            let r = transcode(&j, &mut None);
            let mut v = serde_json::to_value(&r).unwrap();
            v["source"] = json!(tag);
            log(&v);
            proxies.push(v);
        }
        report["proxies"] = json!(proxies);
    }

    if want("thumbnails") {
        // 7. Thumbnails.
        let mut thumbs = Vec::new();
        for src in [&src_1080, &src_4k10] {
            for keyframes in [true, false] {
                let v =
                    tools::thumbnails(src, 60, keyframes).unwrap_or_else(|e| json!({ "error": e }));
                log(&v);
                thumbs.push(v);
            }
        }
        report["thumbnails"] = json!(thumbs);
    }

    if want("export") {
        // 8. Full export with a mixed audio track (source audio + 44.1 kHz music).
        let export_out = o("export-1080p-h264-aac.mp4");
        let t_mix = std::time::Instant::now();
        let track = audio::AudioTrack::build(
            &[
                (src_1080.as_str(), 0.8),
                (p("tests/media/s2/music-44k1.m4a").as_str(), 0.4),
            ],
            60.0,
            192_000,
        );
        let mix_wall = t_mix.elapsed().as_secs_f64();
        let mut export = json!({});
        match track {
            Ok(track) => {
                export["audio"] = json!({ "mixAndDecodeS": track.mix_seconds, "aacEncodeS": track.encode_wall_s, "totalS": mix_wall });
                let mut j = job(
                    &src_1080,
                    &export_out,
                    "h264_nvenc",
                    Mode::HwDownload,
                    Device::D3d11va,
                    12_000_000,
                    usize::MAX,
                    false,
                );
                j.max_frames = None;
                j.audio = Some(track);
                let r = transcode(&j, &mut None);
                export["video"] = serde_json::to_value(&r).unwrap();
                export["probe"] = ffprobe(&export_out);
            }
            Err(e) => export["error"] = json!(e),
        }
        log(&export);
        report["export"] = export;
    }

    // 9. Encoders and DLLs actually present, for the P0-05b trimmed build.
    report["finished"] = json!(now());
    std::fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("report: {report_path}");
}

fn now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("unix:{secs}")
}
