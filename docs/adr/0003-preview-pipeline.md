# ADR-0003 — Pipeline preview & sinkronisasi A/V

- **Status:** Proposed — divalidasi oleh **Spike S1** (lihat [04-phase0-plan.md](../04-phase0-plan.md))

## Konteks
Preview adalah jalur paling sensitif terhadap performa. Mengirim frame mentah dari Rust ke WebView tidak layak
(1 frame 4K RGBA ≈ 33 MB → ~1 GB/s pada 30 fps). Preview dan ekspor harus menghasilkan gambar yang identik.

## Keputusan (usulan)
**Opsi A — "Demux di Rust, decode & composite di WebView":**
1. Rust (`kinora-media`) melakukan *demux* dengan FFmpeg dan mengirim **paket terenkode** + konfigurasi decoder (codec string, `avcC`/`hvcC`) melalui `tauri::ipc::Channel` (biner).
2. UI membuat `VideoDecoder` WebCodecs per sumber (hardware-accelerated), menyimpan `VideoFrame` di ring buffer kecil.
3. Compositor **WebGPU** memakai `importExternalTexture` + shader **WGSL bersama** (`effects/*/effect.wgsl`) — shader yang sama dikompilasi wgpu untuk ekspor.
4. **Audio di Rust** (mixer + `cpal`) adalah *master clock*; Rust mengirim `clock.tick` (posisi rasional + timestamp monotonik), UI memilih frame terdekat.
5. Codec yang tidak didukung WebCodecs (ProRes, DNxHR, MPEG-2, dsb.) → **proxy otomatis** (H.264 all-intra / short-GOP).
6. Scrubbing: decoder di-*flush* + seek ke keyframe terdekat oleh Rust; proxy short-GOP mempercepat scrub.

**Opsi B (cadangan):** wgpu Rust menggambar ke HWND anak di bawah region transparan WebView2.
**Opsi C (fallback driver):** compositor WebGL2 bila WebGPU di-blocklist.

## Kriteria penerimaan (dari Spike S1)
- 1080p30 H.264, 3 layer + 2 efek, di iGPU Intel UHD 620-class: ≥ 30 fps stabil, frame drop < 1%.
- 4K60 HEVC 10-bit di GPU diskrit: ≥ 50 fps; di iGPU via proxy: ≥ 30 fps.
- Drift A/V ≤ 1 frame setelah 10 menit playback; seek acak ≤ 150 ms (proxy).
- Selisih frame preview vs ekspor: PSNR ≥ 40 dB pada fixture efek.

## Konsekuensi
- ➕ Bandwidth IPC kecil (setara bitrate video), decode hardware gratis dari Chromium.
- ➖ Dua runtime GPU (WebGPU & wgpu) — dijaga oleh uji kesetaraan golden-frame.
- ➖ Bergantung pada versi WebView2 evergreen; perlu *feature detection* saat start.
