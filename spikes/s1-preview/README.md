# Spike S1: preview pipeline

Throwaway prototype for [ADR-0003](../../docs/adr/0003-preview-pipeline.md), option A:
**Rust demuxes, WebCodecs decodes, WebGPU composites**, with the audio device as the master clock.
Results: [`docs/spikes/S1-report.md`](../../docs/spikes/S1-report.md).

```
FFmpeg demux (Rust) ──encoded packets──▶ VideoDecoder (WebCodecs, HW) ──VideoFrame──▶ importExternalTexture
      │                 ipc::Response                                                     │
      │                                                                        WebGPU compositor (WGSL)
FFmpeg audio decode ──▶ cpal / WASAPI ──clock──▶ requestAnimationFrame picks the frame covering the clock
```

| File | Role |
|---|---|
| `src-tauri/src/demux.rs` | Demux, WebCodecs codec string from avcC/hvcC/av1C, packet wire format |
| `src-tauri/src/audio.rs` | Audio playback and the master clock (DAC-time compensated) |
| `src-tauri/src/parity.rs` + `parity.wgsl` | wgpu "exporter": same frame, same shared WGSL, PSNR against the preview |
| `src/shaders/effects.wgsl` | Shared effect library used by both runtimes |
| `src/layer.ts` | One WebCodecs decoder per layer, pull-based packet pump, frame-accurate seek |
| `src/compositor.ts` | WebGPU compositor: background + blur, PiP layers, title overlay |
| `src/player.ts` | Clock extrapolation and the render loop |
| `src/bench.ts` | Scenario runner, metrics, ADR-0003 evaluation, JSON report |

## Run

From the repository root, once: `corepack pnpm run ffmpeg:fetch`, then
`node spikes/s1-preview/scripts/make-media.mjs` (writes `tests/media/s1/`).

```sh
cd spikes/s1-preview
corepack pnpm install --ignore-workspace
corepack pnpm run build
cd src-tauri && cargo build --release --features custom-protocol && cd ..
./src-tauri/target/release/s1-preview.exe                            # interactive
./src-tauri/target/release/s1-preview.exe --bench bench/full.json    # benchmark, writes tests/media/s1-reports/
```

Benchmark mode keeps the window on top so Chromium never throttles it.
