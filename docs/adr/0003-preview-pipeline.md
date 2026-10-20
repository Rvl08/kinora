# ADR-0003 — Pipeline preview & sinkronisasi A/V

- **Status:** **Accepted untuk GPU diskrit** (19 Okt 2026, hasil [Spike S1](../spikes/S1-report.md)). Penerimaan penuh menunggu validasi iGPU di Gate P0-13.

## Konteks
Preview adalah jalur paling sensitif terhadap performa. Mengirim frame mentah dari Rust ke WebView tidak layak
(1 frame 4K RGBA ≈ 33 MB → ~1 GB/s pada 30 fps). Preview dan ekspor harus menghasilkan gambar yang identik.

## Keputusan (usulan)
**Opsi A — "Demux di Rust, decode & composite di WebView":**
1. Rust (`kinora-media`) melakukan *demux* dengan FFmpeg dan mengirim **paket terenkode** + konfigurasi decoder (codec string, `avcC`/`hvcC`/`av1C`) sebagai respons biner `tauri::ipc::Response` atas permintaan *pull* dari UI (S1: lebih sederhana daripada `Channel` dan memberi backpressure alami).
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

## Hasil validasi (Spike S1, RTX 5070 Ti, WebView2 155)

| Kriteria | Hasil | Status |
|---|---|---|
| 1080p30 × 3 layer + 2 efek ≥ 30 fps, drop < 1% | 30,0 fps tiap layer, drop 0%, < 2% CPU mesin | ✅ (GPU diskrit) / ❔ iGPU |
| 4K60 HEVC 10-bit ≥ 50 fps | 59,96 fps, drop 0% | ✅ |
| Drift A/V ≤ 1 frame setelah 10 menit | 18.000/18.000 frame, 0 terlambat, drift clock 0,25 ms/menit | ✅ |
| Seek acak ≤ 150 ms (proxy) | proxy GOP-10 × 3: p95 62 ms; file GOP panjang: 0,3–2,3 s → proxy wajib | ✅ |
| PSNR preview vs ekspor ≥ 40 dB | 95,8–97,1 dB (tanpa efek), 67,4–68,0 dB (adjust + blur) | ✅ |

**Penyesuaian keputusan dari temuan S1:**
- Transport paket: respons biner berbasis *pull* (lihat butir 1), batch 4–32 paket per panggilan (latensi ±4 ms/panggilan).
- Seleksi frame: frame tampil = frame terakhir dengan timestamp ≤ clock, dipastikan oleh frame berikutnya (bukan durasi), agar benar untuk VFR.
- Antrean decoder dua mode (pendek saat playback, panjang saat seek) + *watchdog* seek.
- WGSL bersama diorganisasi sebagai **pustaka fungsi murni**; tiap runtime punya entry point sendiri (browser: `texture_external`; wgpu: plane Y/U/V + konversi BT.709 eksplisit).

Detail lengkap, angka mentah, dan syarat produksi: [S1-report.md](../spikes/S1-report.md).
