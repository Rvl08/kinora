//! ONNX Runtime setup shared by the cutout, VAD and TTS tests.

use std::path::Path;
use std::sync::OnceLock;

use ort::environment::Environment;
use ort::session::Session;

pub fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// One environment per process. Telemetry is off: Microsoft's own ONNX Runtime
/// builds send it by default, and Kinora promises no network use it did not ask for.
pub fn environment() -> Result<Environment, String> {
    ort::init()
        .with_name("kinora-s3")
        .with_telemetry(false)
        .build()
        .map_err(err)
}

/// `bench --threads N` overrides every CPU session's thread count.
pub static CPU_THREADS: OnceLock<usize> = OnceLock::new();
/// `bench --lean`: CPU sessions without the arena allocator or memory pattern,
/// trading some speed for a lower peak.
pub static LEAN: OnceLock<bool> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Device {
    Cpu,
    DirectMl,
}

/// A session on the CPU or on DirectML. DirectML fails loudly instead of
/// silently falling back to the CPU, so a measurement says what it measured.
pub fn session(
    env: &Environment,
    model: &Path,
    device: Device,
    threads: usize,
) -> Result<Session, String> {
    let mut b = Session::builder(env).map_err(err)?;
    match device {
        Device::Cpu => {
            b = b
                .with_intra_threads(*CPU_THREADS.get().unwrap_or(&threads))
                .map_err(err)?;
            if LEAN.get() == Some(&true) {
                b = b.with_memory_pattern(false).map_err(err)?;
                b = b
                    .with_execution_providers([ort::ep::CPU::default()
                        .with_arena_allocator(false)
                        .build()])
                    .map_err(err)?;
            }
        }
        Device::DirectMl => {
            // DirectML requires sequential execution and no memory pattern.
            b = b.with_memory_pattern(false).map_err(err)?;
            b = b.with_parallel_execution(false).map_err(err)?;
            b = b
                .with_execution_providers([ort::ep::DirectML::default().build().error_on_failure()])
                .map_err(err)?;
        }
    }
    b.commit_from_file(model).map_err(err)
}

/// "name: type" for every input and output, for the report and for debugging
/// exported models whose tensor names differ between exports.
pub fn signature(s: &Session) -> (Vec<String>, Vec<String>) {
    let ins = s
        .inputs()
        .iter()
        .map(|o| format!("{}: {:?}", o.name(), o.dtype()))
        .collect();
    let outs = s
        .outputs()
        .iter()
        .map(|o| format!("{}: {:?}", o.name(), o.dtype()))
        .collect();
    (ins, outs)
}
