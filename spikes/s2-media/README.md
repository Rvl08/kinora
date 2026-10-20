# Spike S2: media decode/encode paths

Throwaway command-line prototype answering [04-phase0-plan.md](../../docs/04-phase0-plan.md) §4 S2:
which encoders work, how fast an export runs on each pipeline shape, what quality each encoder gives at the
same bitrate, and which FFmpeg components the trimmed release build (P0-05b) must keep.
Results: [`docs/spikes/S2-report.md`](../../docs/spikes/S2-report.md).

| File | Role |
|---|---|
| `src/pipeline.rs` | One export: decode (software / D3D11VA / CUDA) → optional GPU composite → encode → mux, in four modes |
| `src/composite.rs` + `composite.wgsl` | Export-side wgpu composite: NV12 → shared WGSL effects (from S1) → NV12 |
| `src/audio.rs` | Minimal mixer: decode, resample to 48 kHz, sum with gains, AAC |
| `src/tools.rs` | Encoder probe, thumbnails, OpenH264 through Cisco's DLL, VMAF/PSNR/SSIM via libvmaf |
| `src/hw.rs` | FFI helpers: hardware devices, frame download, raw encoder fields |
| `src/main.rs` | Benchmark orchestration, JSON report |

## Run

From the repository root, once: `corepack pnpm run ffmpeg:fetch`, the S1 media
(`node spikes/s1-preview/scripts/make-media.mjs`) and the S2 media (recipe in the S2 report, §2).
OpenH264 needs Cisco's `openh264-2.6.0-win64.dll` in `vendor/openh264/` (see the report for its verification).

```sh
cd spikes/s2-media
cargo build --release
./target/release/s2-media.exe bench    # writes tests/media/s2-out/report.json
./target/release/s2-media.exe bench out.json quality,modes   # only some sections
./target/release/s2-media.exe openh264-debug                  # which OpenH264 setting the DLL rejects
```

Sections: `speed1080p`, `speed4k`, `modes`, `quality`, `proxies`, `thumbnails`, `export` (the encoder probe always runs).
