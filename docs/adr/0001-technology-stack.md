# ADR-0001 — Stack teknologi

- **Status:** Accepted (19 Okt 2026), baris AI diperbarui 21 Okt 2026 ([ADR-0006](0006-local-ai-runtime.md)) · **Keputusan terkait:** D3
- **Pemilik:** Tech lead

## Konteks
Aplikasi harus: (1) ringan di desktop Windows, (2) didistribusikan sebagai `setup.exe` tanpa prasyarat,
(3) mampu memproses video 4K dan AI on-device, (4) cepat dikembangkan oleh tim kecil.
CapCut memakai Qt6 + CEF (±1,6 GB). Concat memakai Rust + Slint (iterasi UI lambat, biner ±136 MB).

## Keputusan
| Lapisan | Pilihan | Versi minimum yang akan dipin di Fase 0 |
|---|---|---|
| Shell | Tauri 2 | 2.x stabil terbaru |
| UI | Svelte 5 + TypeScript (strict) + Vite | Svelte 5.x |
| Paket JS | pnpm (via `corepack`) | — |
| Engine | Rust edition 2024, workspace multi-crate | toolchain stable dipin di `rust-toolchain.toml` |
| Media | FFmpeg 8.1 LGPL shared via `ffmpeg-next` atau `ffmpeg-the-third` (dipilih di Spike S2; lihat ADR-0005 Amandemen 1) | 8.1.3 |
| GPU | wgpu (DX12 utama, Vulkan cadangan), shader WGSL | — |
| Audio | `cpal` (WASAPI) | — |
| AI | `ort` (ONNX Runtime + DirectML), `whisper-rs` (whisper.cpp, Vulkan); TTS Kokoro lewat `ort` dengan G2P sendiri (sherpa-onnx dihapus, [ADR-0006](0006-local-ai-runtime.md)) | ort 2.0.0-rc.15, whisper-rs 0.16 |
| Data | JSON (proyek) + SQLite `rusqlite` (cache/indeks) | — |
| Uji | `cargo nextest`, Vitest, `tauri-driver` (E2E) | — |

## Konsekuensi
- ➕ Installer kecil (WebView2 sudah ada di Windows 10/11), iterasi UI cepat (hot reload), satu bahasa native (Rust).
- ➕ WGSL bisa dipakai bersama oleh WebGPU (preview) dan wgpu (ekspor) — lihat ADR-0003.
- ➖ Dua bahasa (Rust + TS) → batas IPC harus didesain rapi (kontrak tipe dibangkitkan otomatis, mis. `specta`/`ts-rs`).
- ➖ Bergantung pada kualitas WebGPU/WebCodecs di WebView2 → di-de-risk oleh Spike S1.

## Alternatif yang ditolak
- **Electron:** RAM & ukuran besar, bertentangan dengan tujuan "ringan".
- **Qt/C++:** lisensi komersial/LGPL rumit, risiko memori, iterasi lambat.
- **Slint (seperti Concat):** UI kompleks (timeline, inspector) lambat dikembangkan; tidak memberi pembeda.
- **React:** layak, tetapi runtime & pola re-render lebih berat untuk UI padat; Svelte 5 dipilih (D3).
