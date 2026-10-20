//! Spike S1 host: demux + audio clock + measurement plumbing.
//!
//! Interactive: `cargo run --release --features custom-protocol`
//! Benchmark:   same, with `-- --bench <config.json>`; the WebView runs every
//! scenario in the config, writes the JSON report it names and exits.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod demux;
mod parity;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{Manager, State, ipc};

struct App {
    next_id: AtomicU32,
    sources: Mutex<HashMap<u32, Arc<Mutex<demux::Source>>>>,
    audio: audio::AudioEngine,
    sys: Mutex<System>,
    config: Value,
}

fn source(app: &App, id: u32) -> Result<Arc<Mutex<demux::Source>>, String> {
    app.sources
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("no source {id}"))
}

#[tauri::command]
fn get_config(app: State<'_, App>) -> Value {
    app.config.clone()
}

#[tauri::command]
async fn open_source(app: State<'_, App>, path: String) -> Result<demux::SourceInfo, String> {
    let id = app.next_id.fetch_add(1, Ordering::SeqCst);
    let src = demux::Source::open(id, &path)?;
    let info = src.info.clone();
    app.sources
        .lock()
        .unwrap()
        .insert(id, Arc::new(Mutex::new(src)));
    Ok(info)
}

#[tauri::command]
async fn close_source(app: State<'_, App>, id: u32) -> Result<(), String> {
    app.sources.lock().unwrap().remove(&id);
    Ok(())
}

/// Encoded packets in the wire format documented in `demux`.
#[tauri::command]
async fn read_packets(
    app: State<'_, App>,
    id: u32,
    max_bytes: usize,
    max_packets: usize,
) -> Result<ipc::Response, String> {
    let src = source(&app, id)?;
    let mut out = Vec::with_capacity(max_bytes + demux::HEADER_BYTES * 4);
    src.lock()
        .unwrap()
        .read_packets(max_bytes, max_packets, &mut out);
    Ok(ipc::Response::new(out))
}

#[tauri::command]
async fn seek_source(app: State<'_, App>, id: u32, t_us: i64) -> Result<(), String> {
    source(&app, id)?.lock().unwrap().seek(t_us)
}

#[tauri::command]
fn audio_load(app: State<'_, App>, path: String) {
    app.audio.load(path);
}
#[tauri::command]
fn audio_play(app: State<'_, App>) {
    app.audio.play();
}
#[tauri::command]
fn audio_pause(app: State<'_, App>) {
    app.audio.pause();
}
#[tauri::command]
fn audio_seek(app: State<'_, App>, t_us: i64) {
    app.audio.seek(t_us);
}
#[tauri::command]
fn clock(app: State<'_, App>) -> audio::Clock {
    app.audio.clock()
}

/// CPU (% of one core, summed) and resident memory of this process and its
/// WebView2 descendants.
#[tauri::command]
async fn system_stats(app: State<'_, App>) -> Result<Value, String> {
    let mut sys = app.sys.lock().unwrap();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let me = sysinfo::get_current_pid()?;
    let procs = sys.processes();
    let in_tree = |mut pid: Pid| -> bool {
        for _ in 0..16 {
            if pid == me {
                return true;
            }
            match procs.get(&pid).and_then(|p| p.parent()) {
                Some(parent) => pid = parent,
                None => return false,
            }
        }
        false
    };
    let (mut host_cpu, mut host_mem, mut web_cpu, mut web_mem, mut web_n) =
        (0.0, 0u64, 0.0, 0u64, 0);
    for (pid, p) in procs {
        if *pid == me {
            host_cpu += p.cpu_usage();
            host_mem += p.memory();
        } else if in_tree(*pid) {
            web_cpu += p.cpu_usage();
            web_mem += p.memory();
            web_n += 1;
        }
    }
    Ok(json!({
        "hostCpu": host_cpu, "hostMemMb": host_mem as f64 / 1048576.0,
        "webviewCpu": web_cpu, "webviewMemMb": web_mem as f64 / 1048576.0, "webviewProcesses": web_n,
        "logicalCpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
    }))
}

/// Reference frame for the preview/export parity check (see `parity`).
#[tauri::command]
async fn parity_compare(request: ipc::Request<'_>) -> Result<Value, String> {
    let header = |k: &str| -> Result<String, String> {
        request
            .headers()
            .get(k)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
            .ok_or_else(|| format!("missing header {k}"))
    };
    let path = percent_decode(&header("x-path")?);
    let t_us: i64 = header("x-t-us")?
        .parse()
        .map_err(|e| format!("x-t-us: {e}"))?;
    let variant = header("x-variant")?;
    let ipc::InvokeBody::Raw(browser) = request.body() else {
        return Err("expected raw RGBA body".into());
    };
    parity::compare(&path, t_us, &variant, browser)
}

/// Decodes `encodeURIComponent` output (headers must stay ASCII).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[tauri::command]
async fn write_report(path: String, report: Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Walks up from the executable to the repository (the folder with tests/media/s1).
fn media_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .map(|p| p.join("tests").join("media").join("s1"))
        .find(|p| p.is_dir())
}

fn load_config() -> Value {
    let args: Vec<String> = std::env::args().collect();
    let media = media_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(i) = args.iter().position(|a| a == "--bench") {
        let path = args.get(i + 1).expect("--bench needs a config path");
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let mut cfg: Value = serde_json::from_str(&text).expect("bench config is not JSON");
        cfg["mode"] = json!("bench");
        cfg["mediaDir"] = json!(media);
        let base = Path::new(path).parent().unwrap_or(Path::new("."));
        if let Some(out) = cfg["report"].as_str() {
            cfg["report"] = json!(base.join(out).to_string_lossy());
        }
        return cfg;
    }
    json!({ "mode": "interactive", "mediaDir": media })
}

fn main() {
    ffmpeg_next::init().expect("ffmpeg init");
    let config = load_config();
    let bench = config["mode"] == "bench";
    tauri::Builder::default()
        .manage(App {
            next_id: AtomicU32::new(1),
            sources: Mutex::new(HashMap::new()),
            audio: audio::AudioEngine::start(),
            sys: Mutex::new(System::new()),
            config,
        })
        .setup(move |app| {
            if bench {
                if let Some(w) = app.get_webview_window("main") {
                    // Never occluded during a benchmark run.
                    let _ = w.set_always_on_top(true);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            open_source,
            close_source,
            read_packets,
            seek_source,
            audio_load,
            audio_play,
            audio_pause,
            audio_seek,
            clock,
            system_stats,
            parity_compare,
            write_report,
            exit_app
        ])
        .run(tauri::generate_context!())
        .expect("tauri run");
}
