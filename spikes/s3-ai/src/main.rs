//! Spike S3: on-device AI. Throwaway.
//!
//!   s3-ai models                      fetch + verify every model in models/manifest.toml
//!   s3-ai bench [sections] [options]  run measurements (vad,cutout,asr,tts; default all),
//!                                     write tests/media/s3-out/report-<sections>[-<tag>].json
//!
//! Options: --threads N (CPU threads, default 12 for Whisper, 8 for ORT),
//! --cpu-only (skip GPU runs, as on a machine without a usable GPU),
//! --speed-only (TTS without the Whisper round trip), --tag T (report suffix),
//! --model ID and --device cpu|dml (run only matching cutout/TTS configurations,
//! so each can be measured in its own process), --lean (CPU sessions without
//! ORT's arena allocator).

mod asr;
mod audio;
mod cutout;
mod g2p;
mod mem;
mod models;
mod ortx;
mod text;
mod tts;
mod vad;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Value, json};

use audio::Pcm;
use ortx::Device;

fn root() -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root");
    PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\"))
}
fn media(rel: &str) -> PathBuf {
    root().join("tests").join("media").join(rel)
}
fn out_dir() -> PathBuf {
    let d = media("s3-out");
    std::fs::create_dir_all(&d).unwrap();
    d
}
fn store() -> models::Store {
    models::Store::open(
        &root().join("models").join("manifest.toml"),
        &root().join("models").join("cache"),
    )
    .expect("model store")
}
#[derive(Default)]
struct Opts {
    threads: Option<usize>,
    cpu_only: bool,
    speed_only: bool,
    tag: Option<String>,
    model: Option<String>,
    device: Option<Device>,
}
/// Whether a cutout/TTS configuration passes the --model/--device filters.
fn selected(model: &str, device: Device) -> bool {
    let o = opts();
    o.model.as_deref().is_none_or(|m| m == model) && o.device.is_none_or(|d| d == device)
}
static OPTS: OnceLock<Opts> = OnceLock::new();
fn opts() -> &'static Opts {
    OPTS.get_or_init(Opts::default)
}
fn asr_threads() -> i32 {
    opts().threads.unwrap_or(12) as i32
}

fn log(v: &Value) {
    println!("{}", serde_json::to_string(v).unwrap());
}

fn cmd_models() {
    let store = store();
    let mut reports = Vec::new();
    // Resume test first: cut whisper-small short at 50 MB, then let the normal pass resume it.
    match store.fetch("whisper-small", Some(50 * 1024 * 1024)) {
        Ok(r) => reports.push(serde_json::to_value(r).unwrap()),
        Err(e) => reports.push(json!({ "id": "whisper-small", "resumeTestError": e })),
    }
    let ids: Vec<String> = store.models.iter().map(|m| m.id.clone()).collect();
    for id in ids {
        let v = match store.fetch(&id, None) {
            Ok(r) => serde_json::to_value(r).unwrap(),
            Err(e) => json!({ "id": id, "error": e }),
        };
        log(&v);
        reports.push(v);
    }
    std::fs::write(
        out_dir().join("models.json"),
        serde_json::to_string_pretty(&reports).unwrap(),
    )
    .unwrap();
}

/// Ground truth for speech in clean synthetic audio: where the signal leaves
/// and re-enters a -45 dBFS floor (10 ms frames, 120 ms hangover).
fn energy_segments(samples: &[f32], rate: u32) -> Vec<(f64, f64)> {
    let frame = (rate / 100) as usize;
    let floor = 10f32.powf(-45.0 / 20.0);
    let active: Vec<bool> = samples
        .chunks(frame)
        .map(|c| (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt() > floor)
        .collect();
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut quiet = 0;
    for (i, &a) in active.iter().enumerate() {
        if a {
            start.get_or_insert(i);
            quiet = 0;
        } else if let Some(s) = start {
            quiet += 1;
            if quiet > 12 {
                out.push((s as f64 * 10.0, (i - quiet + 1) as f64 * 10.0));
                start = None;
                quiet = 0;
            }
        }
    }
    if let Some(s) = start {
        out.push((s as f64 * 10.0, (active.len() - quiet) as f64 * 10.0));
    }
    out
}

fn bench_vad(env: &ort::environment::Environment, store: &models::Store) -> Value {
    // A "podcast": 0.5 s lead-in, the six English sentences with 1.5 s pauses.
    let rate = 16_000u32;
    let mut samples = vec![0f32; rate as usize / 2];
    for i in 1..=6 {
        let pcm = audio::read_wav(&media(&format!("s3/en/en-{i:02}.wav"))).expect("sapi wav");
        samples.extend_from_slice(&pcm.samples);
        samples.extend(std::iter::repeat_n(0f32, rate as usize * 3 / 2));
    }
    audio::write_wav(
        &out_dir().join("vad-podcast.wav"),
        &Pcm {
            rate,
            samples: samples.clone(),
        },
    )
    .ok();
    let truth = energy_segments(&samples, rate);
    match vad::run(env, &store.path("silero-vad").unwrap(), &samples) {
        Ok(run) => {
            // Match each true segment to the detected segment that overlaps it most.
            let errs: Vec<Value> = truth
                .iter()
                .map(|&(ts, te)| {
                    let best = run.segments.iter().max_by(|a, b| {
                        let ov = |s: &vad::Segment| (s.end_ms.min(te) - s.start_ms.max(ts)).max(0.0);
                        ov(*a).total_cmp(&ov(*b))
                    });
                    match best {
                        Some(s) => json!({ "truth": [ts, te], "detected": [s.start_ms, s.end_ms], "onsetErrMs": s.start_ms - ts, "offsetErrMs": s.end_ms - te }),
                        None => json!({ "truth": [ts, te], "detected": null }),
                    }
                })
                .collect();
            json!({ "truthSegments": truth.len(), "detectedSegments": run.segments.len(), "segments": errs, "run": run })
        }
        Err(e) => json!({ "error": e }),
    }
}

fn bench_cutout(env: &ort::environment::Environment, store: &models::Store) -> Value {
    let imgs: Vec<image::RgbaImage> = ["einstein.jpg", "aldrin.jpg"]
        .iter()
        .map(|f| {
            image::open(media(&format!("s3/img/{f}")))
                .expect("test image")
                .to_rgba8()
        })
        .collect();
    let mut runs = Vec::new();
    let devices: &[Device] = if opts().cpu_only {
        &[Device::Cpu]
    } else {
        &[Device::DirectMl, Device::Cpu]
    };
    let runs_each = if opts().cpu_only { 3 } else { 10 };
    for model in ["birefnet-lite-fp32", "birefnet-lite-fp16"] {
        for &device in devices {
            if !selected(model, device) {
                continue;
            }
            let prefix = out_dir().join(format!("cutout-{model}-{device:?}"));
            let r = cutout::run(
                env,
                &store.path(model).unwrap(),
                device,
                &imgs,
                runs_each,
                Some(&prefix),
            );
            let mut v = serde_json::to_value(&r).unwrap();
            v["memory"] = mem::global().take(mem::Window::Config);
            log(&v);
            runs.push(v);
        }
    }
    json!(runs)
}

struct Clip {
    name: String,
    lang: &'static str,
    pcm: Pcm,
    reference: String,
    words_ms: Vec<(String, f64)>,
}

fn load_clips() -> Vec<Clip> {
    let mut clips = Vec::new();
    for i in 1..=6 {
        let base = media(&format!("s3/en/en-{i:02}"));
        let meta: Value = serde_json::from_str(
            std::fs::read_to_string(base.with_extension("json"))
                .unwrap()
                .trim_start_matches('\u{feff}'),
        )
        .unwrap();
        let mut words: Vec<(String, f64)> = meta["words"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| {
                (
                    w["word"].as_str().unwrap_or("").to_owned(),
                    w["startMs"].as_f64().unwrap_or(0.0),
                )
            })
            .collect();
        words.sort_by(|a, b| a.1.total_cmp(&b.1));
        clips.push(Clip {
            name: format!("en-{i:02}"),
            lang: "en",
            pcm: audio::read_wav(&base.with_extension("wav")).unwrap(),
            reference: meta["text"].as_str().unwrap().to_owned(),
            words_ms: words,
        });
    }
    for i in 1..=25 {
        let base = media(&format!("s3/id/id-{i:02}"));
        let meta: Value =
            serde_json::from_str(&std::fs::read_to_string(base.with_extension("json")).unwrap())
                .unwrap();
        clips.push(Clip {
            name: format!("id-{i:02}"),
            lang: "id",
            pcm: audio::read_wav(&base.with_extension("wav")).unwrap(),
            reference: meta["text"].as_str().unwrap().to_owned(),
            words_ms: vec![],
        });
    }
    clips
}

fn percentile(v: &mut [f64], p: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[((p / 100.0) * (v.len() - 1) as f64).round() as usize])
}

/// Word-start error summary: bias (median signed), |error| p50/p90, share within 80 ms.
fn err_stats(signed: &[f64]) -> Value {
    let mut abs: Vec<f64> = signed.iter().map(|e| e.abs()).collect();
    let within = abs.iter().filter(|&&e| e <= 80.0).count();
    json!({
        "n": signed.len(),
        "biasP50": percentile(&mut signed.to_vec(), 50.0),
        "absP50": percentile(&mut abs.clone(), 50.0),
        "absP90": percentile(&mut abs, 90.0),
        "within80": within as f64 / signed.len().max(1) as f64,
    })
}

fn bench_asr(store: &models::Store) -> Value {
    let clips = load_clips();
    let gpus: &[bool] = if cfg!(feature = "vulkan") && !opts().cpu_only {
        &[true, false]
    } else {
        &[false]
    };
    let mut results = Vec::new();
    for model_id in [
        "whisper-base",
        "whisper-small",
        "whisper-large-v3-turbo-q5_0",
    ] {
        for &gpu in gpus {
            let asr = match asr::Asr::load(&store.path(model_id).unwrap(), model_id, gpu, true) {
                Ok(a) => a,
                Err(e) => {
                    results.push(json!({ "model": model_id, "gpu": gpu, "error": e }));
                    continue;
                }
            };
            for lang in ["en", "id"] {
                let (mut edits, mut ref_words, mut audio_s, mut wall_s) =
                    (0usize, 0usize, 0f64, 0f64);
                // Signed errors (hypothesis - truth, ms) per start estimate.
                let (mut err_t0, mut err_dtw, mut err_prev) = (Vec::new(), Vec::new(), Vec::new());
                let mut samples_out = Vec::new();
                for clip in clips.iter().filter(|c| c.lang == lang) {
                    let t = match asr.transcribe(&clip.pcm.samples, lang, asr_threads()) {
                        Ok(t) => t,
                        Err(e) => {
                            samples_out.push(json!({ "clip": clip.name, "error": e }));
                            continue;
                        }
                    };
                    let r = text::normalize(&clip.reference, lang);
                    let h = text::normalize(&t.text, lang);
                    let (_, e) = text::align(&r, &h);
                    edits += e;
                    ref_words += r.len();
                    audio_s += t.audio_s;
                    wall_s += t.wall_s;
                    // Word timestamps against SAPI's exact word starts (English only).
                    if !clip.words_ms.is_empty() {
                        let truth: Vec<String> = clip
                            .words_ms
                            .iter()
                            .map(|(w, _)| text::normalize(w, "en").join(" "))
                            .collect();
                        let hyp: Vec<String> = t
                            .words
                            .iter()
                            .map(|w| text::normalize(&w.text, "en").join(" "))
                            .collect();
                        let (pairs, _) = text::align(&truth, &hyp);
                        for (ri, hi, same) in pairs {
                            if same {
                                let truth_ms = clip.words_ms[ri].1;
                                let w = &t.words[hi];
                                err_t0.push(w.start_ms - truth_ms);
                                if let Some(d) = w.start_dtw_ms {
                                    err_dtw.push(d - truth_ms);
                                }
                                if let Some(d) = w.start_dtw_prev_ms {
                                    err_prev.push(d - truth_ms);
                                }
                            }
                        }
                    }
                    if samples_out.len() < 3 {
                        samples_out.push(json!({ "clip": clip.name, "reference": clip.reference, "hypothesis": t.text, "wer": text::wer(&r, &h) }));
                    }
                }
                let v = json!({
                    "memory": mem::global().take(mem::Window::Config),
                    "model": model_id, "gpu": gpu, "threads": asr_threads(), "dtw": asr.dtw, "lang": lang, "loadS": asr.load_s,
                    "clips": clips.iter().filter(|c| c.lang == lang).count(),
                    "wer": edits as f64 / ref_words.max(1) as f64,
                    "audioS": audio_s, "wallS": wall_s, "realtimeFactor": wall_s / audio_s.max(1e-9),
                    "wordStartErrMs": { "t0": err_stats(&err_t0), "dtw": err_stats(&err_dtw), "dtwPrev": err_stats(&err_prev) },
                    "samples": samples_out,
                });
                log(&v);
                results.push(v);
            }
        }
    }
    json!(results)
}

fn bench_tts(env: &ort::environment::Environment, store: &models::Store) -> Value {
    let p = |id: &str| store.path(id).unwrap();
    let english = match g2p::English::load(&p("misaki-us-gold"), &p("misaki-us-silver")) {
        Ok(e) => e,
        Err(e) => return json!({ "error": e }),
    };
    let en_texts: Vec<String> = (1..=6)
        .map(|i| {
            let v: Value = serde_json::from_str(
                std::fs::read_to_string(media(&format!("s3/en/en-{i:02}.json")))
                    .unwrap()
                    .trim_start_matches('\u{feff}'),
            )
            .unwrap();
            v["text"].as_str().unwrap().to_owned()
        })
        .collect();
    let id_texts: Vec<String> = (1..=8)
        .map(|i| {
            let v: Value = serde_json::from_str(
                &std::fs::read_to_string(media(&format!("s3/id/id-{i:02}.json"))).unwrap(),
            )
            .unwrap();
            v["text"].as_str().unwrap().to_owned()
        })
        .collect();
    let tts_dir = out_dir().join("tts");
    std::fs::create_dir_all(&tts_dir).ok();
    let mut report = json!({});

    // 1. Speed: model x device on the English set.
    let mut speed = Vec::new();
    let voice = tts::Voice::load(&p("kokoro-voice-af_heart")).unwrap();
    let devices: &[Device] = if opts().cpu_only {
        &[Device::Cpu]
    } else {
        &[Device::DirectMl, Device::Cpu]
    };
    for model in ["kokoro-v1-uint8", "kokoro-v1-fp32"] {
        for &device in devices {
            if !selected(model, device) {
                continue;
            }
            let mut k = match tts::Kokoro::load(env, &p(model), &p("kokoro-tokenizer"), device) {
                Ok(k) => k,
                Err(e) => {
                    speed.push(json!({ "model": model, "device": device, "error": e }));
                    continue;
                }
            };
            let (mut audio_s, mut wall_s, mut first) = (0f64, 0f64, None);
            for t in &en_texts {
                let mut st = g2p::G2pStats::default();
                let ph = english.phonemize(t, &mut st);
                match k.synthesize(&ph, &voice, 1.0) {
                    Ok(s) => {
                        first.get_or_insert(s.wall_s);
                        audio_s += s.pcm.seconds();
                        wall_s += s.wall_s;
                    }
                    Err(e) => {
                        speed.push(json!({ "model": model, "device": device, "error": e }));
                        break;
                    }
                }
            }
            let v = json!({ "memory": mem::global().take(mem::Window::Config), "model": model, "device": device, "loadMs": k.load_ms, "firstS": first, "audioS": audio_s, "wallS": wall_s, "realtimeFactor": wall_s / audio_s.max(1e-9), "signature": k.signature });
            log(&v);
            speed.push(v);
        }
    }
    report["speed"] = json!(speed);
    if opts().speed_only {
        return report;
    }

    // 2. Intelligibility: synthesise, transcribe with Whisper, score the words.
    let mut k = match tts::Kokoro::load(
        env,
        &p("kokoro-v1-fp32"),
        &p("kokoro-tokenizer"),
        Device::Cpu,
    ) {
        Ok(k) => k,
        Err(e) => {
            report["roundTrip"] = json!({ "error": e });
            return report;
        }
    };
    let asr = match asr::Asr::load(
        &p("whisper-large-v3-turbo-q5_0"),
        "whisper-large-v3-turbo-q5_0",
        cfg!(feature = "vulkan"),
        false,
    ) {
        Ok(a) => a,
        Err(e) => {
            report["roundTrip"] = json!({ "error": e });
            return report;
        }
    };
    let mut trips = Vec::new();
    let cases: Vec<(&str, &str, &Vec<String>)> = vec![
        ("en", "af_heart", &en_texts),
        ("en", "am_michael", &en_texts),
        ("id", "ef_dora", &id_texts),
        ("id", "em_alex", &id_texts),
        ("id", "if_sara", &id_texts),
        ("id", "pf_dora", &id_texts),
        ("id", "af_heart", &id_texts),
    ];
    for (lang, voice_id, texts) in cases {
        let voice = tts::Voice::load(&p(&format!("kokoro-voice-{voice_id}"))).unwrap();
        let (mut edits, mut words) = (0usize, 0usize);
        let mut stats = g2p::G2pStats::default();
        let mut dropped: Vec<char> = Vec::new();
        let mut examples = Vec::new();
        for (i, t) in texts.iter().enumerate() {
            let ph = if lang == "en" {
                english.phonemize(t, &mut stats)
            } else {
                g2p::indonesian(t, &mut stats)
            };
            let s = match k.synthesize(&ph, &voice, 1.0) {
                Ok(s) => s,
                Err(e) => {
                    examples.push(json!({ "text": t, "error": e }));
                    continue;
                }
            };
            dropped.extend(&s.dropped);
            if i < 2 {
                audio::write_wav(&tts_dir.join(format!("{lang}-{voice_id}-{i}.wav")), &s.pcm).ok();
            }
            let pcm16 = audio::resample(&s.pcm, 16_000);
            let tr = asr.transcribe(&pcm16.samples, lang, asr_threads());
            let hyp = tr.as_ref().map(|t| t.text.clone()).unwrap_or_default();
            let r = text::normalize(t, lang);
            let h = text::normalize(&hyp, lang);
            edits += text::align(&r, &h).1;
            words += r.len();
            if i < 2 {
                examples.push(
                    json!({ "text": t, "phonemes": ph, "heard": hyp, "wer": text::wer(&r, &h) }),
                );
            }
        }
        dropped.sort_unstable();
        dropped.dedup();
        let v = json!({ "lang": lang, "voice": voice_id, "g2p": if lang == "en" { "misaki dictionary" } else { "Kinora rules (id)" }, "sentences": texts.len(), "wer": edits as f64 / words.max(1) as f64, "g2pStats": stats, "droppedSymbols": dropped.iter().collect::<String>(), "examples": examples });
        log(&v);
        trips.push(v);
    }
    report["roundTrip"] = json!(trips);
    report
}

fn cmd_bench(sections: &str) {
    let store = store();
    let env = ortx::environment().expect("ORT environment");
    let want = |s: &str| sections == "all" || sections.split(',').any(|x| x == s);
    let o = opts();
    let mut report = json!({
        "spike": "S3 AI", "vulkanBuild": cfg!(feature = "vulkan"), "sections": sections,
        "threads": o.threads, "cpuOnly": o.cpu_only, "speedOnly": o.speed_only,
        "logicalCpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
    });
    // Peak RAM/VRAM per section. Run one section per process for clean peaks:
    // models and runtimes freed by a section are not always returned to the OS.
    let sampler = mem::global();
    let mut memory = json!({ "idle": sampler.take(mem::Window::Section) });
    let sections_fns: [(&str, &dyn Fn() -> Value); 4] = [
        ("vad", &|| bench_vad(&env, &store)),
        ("cutout", &|| bench_cutout(&env, &store)),
        ("asr", &|| bench_asr(&store)),
        ("tts", &|| bench_tts(&env, &store)),
    ];
    for (name, f) in sections_fns {
        if want(name) {
            report[name] = f();
            memory[name] = sampler.take(mem::Window::Section);
            log(&json!({ "memory": name, "peaks": memory[name] }));
        }
    }
    report["memory"] = memory;
    let tag = o
        .tag
        .as_deref()
        .map(|t| format!("-{t}"))
        .unwrap_or_default();
    let name = format!("report-{}{tag}.json", sections.replace(',', "_"));
    std::fs::write(
        out_dir().join(&name),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("report: {name}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("models") => cmd_models(),
        Some("bench") => {
            let mut sections = "all";
            let mut o = Opts::default();
            let mut it = args[2..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--threads" => o.threads = it.next().and_then(|n| n.parse().ok()),
                    "--cpu-only" => o.cpu_only = true,
                    "--speed-only" => o.speed_only = true,
                    "--tag" => o.tag = it.next().cloned(),
                    "--model" => o.model = it.next().cloned(),
                    "--lean" => {
                        ortx::LEAN.set(true).ok();
                    }
                    "--device" => {
                        o.device = match it.next().map(String::as_str) {
                            Some("cpu") => Some(Device::Cpu),
                            Some("dml") => Some(Device::DirectMl),
                            other => panic!("--device cpu|dml, got {other:?}"),
                        }
                    }
                    s => sections = s,
                }
            }
            if let Some(n) = o.threads {
                ortx::CPU_THREADS.set(n).ok();
            }
            OPTS.set(o).ok();
            cmd_bench(sections)
        }
        _ => {
            eprintln!("usage: s3-ai models | bench [vad,cutout,asr,tts]");
            std::process::exit(2);
        }
    }
}
