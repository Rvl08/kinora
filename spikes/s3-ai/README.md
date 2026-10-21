# Spike S3: on-device AI runtime

Throwaway command-line prototype answering [04-phase0-plan.md](../../docs/04-phase0-plan.md) §4 S3:
whether ONNX Runtime + DirectML and whisper.cpp run on the GPU with nothing but the GPU driver installed,
how fast and how accurate they are, how much memory they take, and whether TTS works without espeak-ng.
Results: [`docs/spikes/S3-report.md`](../../docs/spikes/S3-report.md). Decision: [ADR-0006](../../docs/adr/0006-local-ai-runtime.md).

| File | Role |
|---|---|
| `src/models.rs` | Model manager: pinned URLs from `models/manifest.toml`, resumable download (HTTP Range → `.part`), SHA-256 / git-blob verification, atomic rename |
| `src/ortx.rs` | ONNX Runtime environment (telemetry off) and CPU/DirectML sessions |
| `src/vad.rs` | Silero VAD v5 and the reference segmenter |
| `src/cutout.rs` | BiRefNet-lite matting: preprocess, inference, mask and cutout PNGs |
| `src/asr.rs` | whisper.cpp via `whisper-rs`: text, word starts (token t0, DTW, DTW of the previous token) |
| `src/tts.rs` | Kokoro-82M through ORT directly (no sherpa-onnx), voice style vectors |
| `src/g2p.rs` | Text → phonemes without espeak-ng: misaki dictionaries (English), spelling rules (Indonesian) |
| `src/text.rs` | Number expansion, normalisation, word alignment, WER |
| `src/audio.rs` | WAV I/O and resampling |
| `src/mem.rs` | Peak RAM (working set, commit) and per-process VRAM (DXGI), sampled every 50 ms |
| `src/main.rs` | `models` and `bench` commands, JSON reports |
| `sandbox/` | Windows Sandbox runner (see below) |
| `scripts/make-speech.ps1` | English test sentences with exact word timings (Windows SAPI) |

## Why it runs in Windows Sandbox

The dev machine enforces Smart App Control, which blocks freshly built unsigned executables, Cargo build
scripts included. `sandbox/prepare.ps1` writes a `.wsb` that maps the repository to `C:\k`, the host's
toolchains read-only, and a build cache, then `sandbox/run.ps1` builds and runs the tasks inside the
sandbox and shuts it down. The short paths also keep whisper.cpp's CMake build under MAX_PATH.

## Run

Test media (not committed): `tests/media/s3/id` (25 FLEURS `id_id` dev clips, CC-BY-4.0, 16 kHz WAV + JSON text),
`tests/media/s3/en` (`scripts/make-speech.ps1`), `tests/media/s3/img` (two public-domain photos, sources in `SOURCES.json`).
Build tools on the host: Visual Studio (C++), CMake, LLVM (`libclang` for bindgen), the Vulkan SDK (for `--features vulkan`)
and `vendor/tools/ninja.exe` (Ninja 1.13.2 release zip from GitHub, SHA-256 checked; not committed).

```powershell
# Host: prepare a session, then start it. Tasks are separated by ';', each runs in its own process.
powershell -ExecutionPolicy Bypass -File spikes/s3-ai/sandbox/prepare.ps1 -Task "models;bench vad;bench asr" -Features vulkan
start spikes/s3-ai/sandbox/generated/kinora-s3.wsb
# Output: tests/media/s3-out/ (report-*.json, task-*.log, build.log, sandbox.done)
```

Commands (inside the sandbox, or on a machine without Smart App Control):

```sh
s3-ai models                          # fetch + verify all models (includes a resume test)
s3-ai bench vad,cutout,asr,tts        # sections; report-<sections>.json
s3-ai bench cutout --model birefnet-lite-fp16 --device dml --tag fp16-dml   # one configuration
s3-ai bench asr,tts --threads 4 --cpu-only --speed-only --tag lowspec        # weak-CPU simulation
```

Options: `--threads N`, `--cpu-only`, `--speed-only` (TTS without the Whisper round trip), `--model ID`,
`--device cpu|dml`, `--lean` (CPU sessions without ORT's arena allocator), `--tag T` (report name suffix).
The `vulkan` Cargo feature builds whisper.cpp's Vulkan backend (needs the Vulkan SDK on the build machine).
