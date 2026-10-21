//! Peak memory per bench section: this process's RAM (working set and commit)
//! and its local video memory. Windows only reports current use for video
//! memory, so a thread samples both every 50 ms and keeps the maximum.
//!
//! Video memory comes from DXGI's per-process budget query. WDDM counts every
//! allocation the process makes on that adapter, so D3D12 (DirectML) and
//! Vulkan (whisper.cpp) are both included.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde_json::{Value, json};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
    DXGI_QUERY_VIDEO_MEMORY_INFO, IDXGIAdapter3, IDXGIFactory1,
};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::core::Interface;

const MB: f64 = 1024.0 * 1024.0;

static GLOBAL: OnceLock<Sampler> = OnceLock::new();

/// The process-wide sampler, started on first use.
pub fn global() -> &'static Sampler {
    GLOBAL.get_or_init(Sampler::start)
}

#[derive(Default)]
struct Peaks {
    working_set: AtomicU64,
    commit: AtomicU64,
    vram: AtomicU64,
}

/// Two independent peak windows, so per-configuration snapshots do not reset
/// the per-section figure.
#[derive(Clone, Copy)]
pub enum Window {
    Config = 0,
    Section = 1,
}

pub struct Sampler {
    peaks: Arc<[Peaks; 2]>,
    vram_ok: Arc<AtomicBool>,
}

fn ram() -> (u64, u64) {
    let mut c = PROCESS_MEMORY_COUNTERS {
        cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    // SAFETY: c is a valid, correctly sized PROCESS_MEMORY_COUNTERS.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) }.as_bool();
    if ok {
        (c.WorkingSetSize as u64, c.PagefileUsage as u64)
    } else {
        (0, 0)
    }
}

/// The hardware adapter with the most dedicated memory (the one the models run on).
fn adapter() -> Option<IDXGIAdapter3> {
    // SAFETY: plain DXGI factory/adapter enumeration.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut best: Option<(usize, IDXGIAdapter3)> = None;
        for i in 0.. {
            let Ok(a) = factory.EnumAdapters1(i) else {
                break;
            };
            let Ok(d) = a.GetDesc1() else { continue };
            if d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|(m, _)| d.DedicatedVideoMemory > *m)
            {
                if let Ok(a3) = a.cast::<IDXGIAdapter3>() {
                    best = Some((d.DedicatedVideoMemory, a3));
                }
            }
        }
        best.map(|(_, a)| a)
    }
}

fn vram(a: &IDXGIAdapter3) -> Option<u64> {
    let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
    // SAFETY: info is a valid out pointer.
    unsafe { a.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info) }.ok()?;
    Some(info.CurrentUsage)
}

impl Sampler {
    /// Starts the sampling thread; it runs until the process exits.
    fn start() -> Self {
        let peaks: Arc<[Peaks; 2]> = Arc::new(Default::default());
        let vram_ok = Arc::new(AtomicBool::new(false));
        let (p, v) = (peaks.clone(), vram_ok.clone());
        // DXGI interfaces are not Send: the adapter lives on the sampling thread.
        std::thread::spawn(move || {
            let a = adapter();
            loop {
                let (ws, commit) = ram();
                let used = a.as_ref().and_then(vram);
                if used.is_some() {
                    v.store(true, Ordering::Relaxed);
                }
                for w in p.iter() {
                    w.working_set.fetch_max(ws, Ordering::Relaxed);
                    w.commit.fetch_max(commit, Ordering::Relaxed);
                    w.vram.fetch_max(used.unwrap_or(0), Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        Self { peaks, vram_ok }
    }

    /// Peaks in `window` since its previous call, in MB; resets that window.
    pub fn take(&self, window: Window) -> Value {
        std::thread::sleep(Duration::from_millis(60)); // let the sampler see the end state
        let (ws, commit) = ram();
        let p = &self.peaks[window as usize];
        json!({
            "workingSetPeakMb": p.working_set.swap(ws, Ordering::Relaxed) as f64 / MB,
            "commitPeakMb": p.commit.swap(commit, Ordering::Relaxed) as f64 / MB,
            "vramPeakMb": self.vram_ok.load(Ordering::Relaxed).then(|| p.vram.swap(0, Ordering::Relaxed) as f64 / MB),
        })
    }
}
