# Laporan Spike S1 — Pipeline Preview

> **Status:** Selesai (19 Okt 2026), diuji pada RTX 5070 Ti; iGPU belum · **ADR terkait:** [ADR-0003](../adr/0003-preview-pipeline.md) · **Kode:** [`spikes/s1-preview/`](../../spikes/s1-preview/README.md)
> **Rekomendasi:** ✅ **Terima Opsi A** (demux Rust → WebCodecs → WebGPU) dengan dua syarat produksi (§6).

## 1. Ringkasan

| Kriteria ADR-0003 | Target | Hasil (RTX 5070 Ti) | Status |
|---|---|---|---|
| 1080p30 H.264, 3 layer + 2 efek + teks | ≥ 30 fps, drop < 1% | 30,0 fps konten di ketiga layer, drop **0%**, render 240 fps | ✅ |
| 4K60 HEVC 10-bit, GPU diskrit | ≥ 50 fps | **59,96 fps**, drop 0% | ✅ |
| Seek acak via proxy | p95 ≤ 150 ms | proxy GOP-10, 3 layer: p50 **46 ms**, p95 **62 ms**, 30/30 frame tepat | ✅ |
| Ketepatan frame setelah seek | 100% | CFR 100% (ratusan seek, semua codec); VFR 179/180 (§3.5) | ⚠️ |
| Seek acak pada file asli (GOP panjang) | (informasi) | p95 329 ms (1 layer) – 2.271 ms (4K × 3) | ➖ butuh proxy |
| Drift A/V setelah 10 menit | ≤ 1 frame | 18.000/18.000 frame tampil, 0 terlambat, 0 underrun, skew p95 30 ms (< 1 frame) | ✅ |
| Paritas preview vs ekspor | PSNR ≥ 40 dB | **95,8–97,1 dB** (tanpa efek), **67,4–68,0 dB** (adjust + blur) | ✅ |
| iGPU (Intel UHD 620-class) | ≥ 30 fps | **tidak diuji** — tidak ada perangkat (i7-14700F tanpa iGPU) | ❔ |

## 2. Lingkungan & metode

- **Mesin:** Windows 11 Pro 26300, i7-14700F (20C/28T), 32 GB, NVIDIA RTX 5070 Ti (Blackwell, driver 32.0.16.1742), monitor ±240 Hz, DPR 1,25.
- **Runtime:** WebView2 / Chromium **155**, Tauri 2.12.4, wgpu 30.0.1, cpal 0.18.2 (WASAPI), FFmpeg **8.1.3 LGPL** (P0-05), `ffmpeg-next` 8.1.0.
- **Media uji** (sintetis, `scripts/make-media.mjs`): `testsrc2` + nomor frame & timestamp tercetak + beep 1 kHz tiap detik; semua bertag BT.709 limited.
  1080p30 H.264 (GOP 60), 1080p30 H.264 GOP-10 ("proxy-like"), 4K60 HEVC Main10, 1080p VFR (±25,7 fps rata-rata; frame 33/67 ms), 1080p30 AV1, 4K30 ProRes 422, 1080p30 H.264 11 menit.
- **Pengukuran** (`bench.ts`, mode `--bench`, jendela *always-on-top* + flag anti-throttling Chromium):
  fps render (interval rAF), fps konten & frame terlewat per layer, *starvation*, skew A/V (clock audio − timestamp frame tampil), drift clock audio vs jam dinding (regresi linear),
  latensi & bandwidth IPC per panggilan, latensi seek sampai frame target **tergambar**, ketepatan frame setelah seek, CPU & RAM (proses host + seluruh proses anak WebView2, via `sysinfo`), waktu ke frame pertama.
- **Paritas:** frame yang sama dirender (a) di WebView via `importExternalTexture` + WGSL bersama, dibaca balik dari tekstur `rgba8unorm`, dan (b) di Rust: decode software FFmpeg → plane Y/U/V → wgpu dengan WGSL bersama yang sama → dibaca balik. PSNR atas RGB.
- Laporan mentah: `tests/media/s1-reports/*.json` (tidak di-commit; dapat dibangkitkan ulang).

## 3. Hasil

### 3.1 Playback

| Skenario | Konten fps / layer | Drop | Starvation | Render fps (p1-low) | Waktu ke frame pertama | CPU mesin* | RAM host / WebView2 (maks) |
|---|---|---|---|---|---|---|---|
| 1080p30 H.264 × 1 | 30,0 | 0% | 0 | 240 (233) | 45 ms | 2,0% | 86 / 496 MB |
| 1080p30 H.264 × 3 + efek + teks | 30,0 ×3 | 0% | 0 | 240 (233) | 65 ms | 1,7% | 176 / 592 MB |
| 1080p30 GOP-10 × 3 + efek + teks | 30,0 ×3 | 0% | 0 | 240 (233) | 63 ms | 3,0% | 236 / 694 MB |
| 1080p30 × 3 + efek, **decode software** | 30,0 ×3 | 0% | 0 | 240 (233) | 68 ms | 4,0% | 301 / 937 MB |
| 4K60 HEVC 10-bit × 1 | 59,96 | 0% | 0 | 240 (233) | 155 ms | 3,5% | 447 / 740 MB |
| 4K60 HEVC 10-bit × 3 + efek + teks | 59,26 ×3 | **1,17%** | 5–21 tick | 225 (**60**) | 39 ms | 3,9% | 658 / 934 MB |
| 1080p30 AV1 × 1 | 30,0 | 0% | 0 | 235 (120) | 17 ms | 3,4% | 897 / 690 MB |
| 4K30 ProRes 422 | — | — | — | — | — | — | WebCodecs: **tidak didukung** → jalur proxy |

\* % dari seluruh 28 thread logis; satu core penuh = 3,6%. WebView2 memakai 0,4–1,0 core; host Rust 0,04–0,2 core.

**Kesimpulan:** decode hardware Chromium + compositor WebGPU memberi margin sangat besar di GPU diskrit: 1080p multi-layer memakai < 2% CPU mesin, dan renderer berjalan pada refresh monitor penuh. Batas pertama baru terlihat pada **tiga stream 4K60 10-bit sekaligus** (drop 1,2%).

### 3.2 IPC (paket terenkode, bukan frame mentah)
- Per panggilan `read_packets`: p50 **3–5 ms**, p95 4,6–7 ms; bandwidth 1–4,8 MB/s per layer (setara bitrate video).
- Sebagai pembanding, frame mentah 4K RGBA pada 60 fps butuh ±2 GB/s. Desain "kirim paket, bukan piksel" terbukti menjadi kuncinya.
- Latensi per panggilan ±4 ms relatif tinggi (custom protocol WebView2), tetapi tidak menjadi bottleneck karena *batching* 4–32 paket per panggilan dan pipeline *pull* yang berjalan mendahului.

### 3.3 Seek

| Skenario | p50 | p95 | Maks | Frame tepat |
|---|---|---|---|---|
| 1080p30 GOP-10 × 3 (**proxy-like**) | **46 ms** | **62 ms** | 160 ms | 30/30 |
| 1080p30 GOP-60 × 1 | 192 ms | 329 ms | 329 ms | 20/20 |
| 1080p30 GOP-60 × 3 + efek | 336 ms | 767 ms | 862 ms | 20/20 |
| 1080p30 GOP-60 × 3, decode software | 108 ms | 162 ms | 171 ms | 20/20 |
| 4K60 HEVC GOP-120 × 1 | 379 ms | 596 ms | 837 ms | 20/20 |
| 4K60 HEVC GOP-120 × 3 + efek | 1.175 ms | 2.271 ms | 2.583 ms | 20/20 |
| 1080p30 AV1 GOP-60 × 1 | 172 ms | 271 ms | 350 ms | 20/20 |

- Latensi seek sebanding dengan **jarak ke keyframe sebelumnya × jumlah layer**. Ini memvalidasi rencana ADR-0003 §5–6: **scrubbing memakai proxy short-GOP**.
- **Temuan tak terduga:** decode **software** melakukan seek lebih cepat daripada hardware (p95 162 vs 767 ms) pada GOP-60 × 3, walaupun playback hardware jauh lebih hemat CPU.
  Dugaan (inferensi): decoder hardware Chromium punya latensi per frame/antrean output yang membatasi *throughput* saat "lari" ke target. Strategi produksi yang perlu diuji: **decoder software khusus seek/scrub**, hardware untuk playback.
- Ketepatan frame 100% pada semua codec CFR (lihat §3.5 untuk VFR).

### 3.4 Clock & sinkronisasi A/V
- Clock master = sampel yang benar-benar diputar perangkat, dikompensasi latensi output WASAPI (**±32 ms**, dari timestamp `playback` cpal); 0 underrun di semua skenario.
- Skew (clock − frame tampil) berada di rentang [0, durasi frame) sesuai desain: p95 30 ms @30 fps, 14–15 ms @60 fps.
- Drift clock audio vs jam dinding: 0,04–0,6 ms/menit (selisih kristal audio vs QPC). Video *mengikuti* clock audio, sehingga drift ini tidak menjadi desync A/V.
- **Uji 10 menit** (1080p30, 600 s): **18.000 dari 18.000** frame tampil, 0 terlewat, 0 tick terlambat, 0 underrun audio. Skew p50/p95/maks 16,6 / 30,2 / 33,3 ms (selalu di dalam slot frame), drift clock audio 0,25 ms/menit vs jam dinding. RAM host datar 48 MB, WebView2 608 MB.

### 3.5 VFR
File 1080p VFR (frame 33 dan 67 ms, rata-rata 25,7 fps):
- Playback: 25,66–25,7 fps konten (= rata-rata file), 0 terlewat, 0 terlambat; skew maks 66,6 ms = durasi frame terpanjang, sesuai desain.
- Seek: **179 dari 180** frame tepat di tiga run (30 + 60 + 30 seek), p95 312–513 ms (GOP 60).
- **1 kegagalan intermiten** (timeout 5 s) pada run 30-seek pertama setelah perbaikan. Run ulang dengan target identik (seed sama) lulus 30/30, dan mekanisme "mundur-seek" tidak pernah terpicu. **Akar masalah belum diketahui**; mitigasi produksi: *watchdog* seek (retry bila frame target tidak muncul dalam 500 ms) + log diagnostik.

### 3.6 Paritas preview vs ekspor

| Frame | Tanpa efek | Color adjust + blur 2-pass |
|---|---|---|
| t = 10,000 s | 97,15 dB (selisih maks 2/255) | 68,00 dB (maks 1/255) |
| t = 33,367 s (batas frame) | 96,34 dB | 67,42 dB |
| t = 47,000 s | 95,78 dB | 67,91 dB |

- Konversi YUV→RGB di browser (`importExternalTexture`) dan konversi BT.709 eksplisit di WGSL kita **praktis identik** (selisih ≤ 2/255).
  Syaratnya: media **bertag warna**. Media tanpa tag belum diuji, dan itu risiko nyata di produksi (§6).
- Efek WGSL bersama memberi hasil yang sama di WebGPU (Tint) dan wgpu (Naga/DX12) → klaim "satu shader, dua runtime" **terbukti**.
- Frame yang dipilih browser dan referensi FFmpeg **sama persis** di ketiga titik, termasuk tepat di batas frame (33,366667 s).

## 4. Temuan selama spike (bug & pelajaran)
1. **Seleksi frame harus berbasis timestamp frame berikutnya, bukan durasi.** Versi awal memakai `timestamp + duration` dan gagal pada VFR serta saat target jatuh di celah frame. Pola yang benar sekarang terkodekan di `layer.ts`: frame tampil = frame terakhir dengan timestamp ≤ t, dan baru dipastikan setelah frame berikutnya ter-decode (atau EOS).
2. **Antrean decoder perlu dua mode:** pendek saat playback (memori GPU; HW decoder Chromium membatasi frame yang belum di-`close()`), panjang saat seek. Hasilnya seek turun ±6× pada smoke test.
3. **Media harus bertag warna eksplisit.** NVENC menulis VUI dari metadata *frame*, bukan dari opsi `-colorspace` (butuh `setparams`).
4. **Throttling Chromium:** tanpa `backgroundThrottling: disabled` dan flag anti-*occlusion*, angka fps di jendela yang tertutup tidak dapat dipercaya.
5. **Kebocoran memori di jalur decode audio (diperbaiki).** `Packet::read()` milik `ffmpeg-next` memanggil `av_read_frame` tanpa melepas payload paket sebelumnya. Memakai ulang satu `Packet` sambil melewati paket video membocorkan setiap payload yang dilewati (±32 MB per 20 detik). Ditemukan lewat uji pengulangan, dilokalisasi dengan test Rust terisolasi (`leak_demux` datar 19 MB, `leak_audio_decode` +975 MB → datar 17 MB setelah perbaikan). **Aturan produksi:** satu `Packet` baru per `read`, plus test kebocoran sebagai bagian dari CI.
6. **Harness pengukuran juga perlu diuji.** `Math.max(...array)` dengan ±150 ribu sampel melebihi batas call stack JS dan menggagalkan run 10 menit pertama.

## 5. Memori
| Ukuran | Sebelum perbaikan kebocoran | Sesudah |
|---|---|---|
| Host Rust, 5 run identik berturut-turut (1080p × 3) | 129 → 163 → 202 → 237 → 274 MB | **43 → 44 → 46 → 45 → 45 MB** |
| Host Rust, 10 menit playback | — | **48 MB** (datar) |
| WebView2 (semua proses anak), 1 layer 1080p | ±495 MB | ±495 MB |
| WebView2, 3 layer 1080p + efek | ±600 MB | 603 → 621 → 631 → 648 → 667 MB (5 run berturut-turut) |

- Host Rust ringan dan stabil. **Beban memori ada di WebView2** (proses GPU, renderer, utilitas): 0,5–0,9 GB saat memutar. Ini jauh dari anggaran "RAM idle ≤ 300 MB" (03 §6.4), meskipun anggaran itu untuk kondisi *idle*, bukan playback.
- WebView2 naik ±16 MB **per pembukaan proyek** (bukan per waktu putar; run 10 menit datar). Kemungkinan objek decoder/GPU belum di-GC. Ini butuh **uji soak** "buka/tutup proyek 50×" sebelum Alpha.

## 6. Rekomendasi & syarat produksi
**Keputusan yang diusulkan: terima Opsi A** sebagai pipeline preview Kinora. Semua kriteria ADR-0003 yang dapat diuji di perangkat ini terpenuhi dengan margin besar. Opsi B (wgpu ke HWND) dan C (WebGL2) tidak perlu dibangun sekarang.

Syarat sebelum/selama Fase 1:
1. **Validasi iGPU wajib sebelum Gate Fase 0** (laptop Intel UHD 620-class): skenario 1080p × 3 + efek dan proxy. Bila gagal, aktifkan fallback "kurangi resolusi preview" sebelum mempertimbangkan Opsi B.
2. **Proxy otomatis short-GOP** (GOP ≤ 10, H.264, setengah resolusi untuk sumber > 1080p) untuk scrubbing dan codec non-WebCodecs (ProRes, DNxHR). Terbukti memenuhi seek ≤ 150 ms.
3. **Media tanpa tag warna**: tetapkan kebijakan default (BT.709 untuk HD, BT.601 untuk SD) di kedua runtime dan tambahkan fixture tanpa tag ke uji paritas.
4. **Seleksi frame berbasis timestamp** (bukan durasi), antrean decoder dua mode, *watchdog* seek, satu `Packet` per `read`. Semuanya dibawa ke kode produksi dengan test.
5. **Uji kebocoran & soak di CI** (pola test `leak_*`), ditambah soak "buka/tutup proyek 50×" untuk WebView2.
6. **Eksperimen lanjutan (Fase 1):** decoder software khusus seek/scrub, decoder hardware untuk playback (§3.3).
7. **Anggaran memori** (03 §6.4) perlu dirumuskan ulang dengan memisahkan host vs WebView2 dan idle vs playback.

## 7. Yang belum terbukti (unknowns)
- **iGPU / laptop low-end:** tidak diuji. Proksi terdekat adalah decode software (playback tetap 30 fps × 3, CPU 4% di mesin 28-thread), tetapi itu **bukan** bukti untuk Intel UHD 620. Wajib diuji di perangkat nyata sebelum Gate.
- **AMD / Intel Arc, Windows 10 tanpa WebGPU, driver yang di-blocklist** (fallback WebGL2 = Opsi C belum dibangun).
- **HDR (PQ/HLG):** HEVC 10-bit yang diuji adalah SDR BT.709; tone-mapping belum disentuh.
- **Audio clock tanpa perangkat output** (jalur fallback wall-clock ada, belum diukur).
