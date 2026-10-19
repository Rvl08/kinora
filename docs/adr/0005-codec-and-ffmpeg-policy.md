# ADR-0005 — Kebijakan codec & build FFmpeg

- **Status:** Accepted (19 Okt 2026) · **Keputusan terkait:** D2

## Konteks
Lisensi proyek permisif (ADR-0002) melarang komponen GPL. FFmpeg berlisensi LGPL-2.1+ bila dibangun tanpa
`--enable-gpl`/`--enable-nonfree`. Paten H.264/HEVC memengaruhi encoder software.

> ⚠️ Catatan lingkungan dev: `FFMPEG_DIR` di mesin saat ini = `D:\Ffmpeg\ffmpeg-master-latest-win64-gpl-shared` (**GPL**, untuk Concat).
> Kinora wajib memakai build **LGPL** terpisah dan menyetel `FFMPEG_DIR` per-proyek (mis. `.cargo/config.toml` `[env]`), bukan global.

## Keputusan
1. **FFmpeg 7.x LGPL shared** (DLL dinamis), dari build reproducible milik proyek (CI) — basis konfigurasi BtbN `lgpl-shared`, dipangkas ke codec yang dibutuhkan.
2. **Decode:** semua decoder bawaan FFmpeg + `dav1d` (BSD) + HW (D3D11VA, DXVA2, NVDEC/CUVID, QSV via libvpl).
3. **Encode:**
   | Format | Hardware (utama) | Software (fallback) |
   |---|---|---|
   | H.264 | NVENC / QSV / AMF | **OpenH264** (biner Cisco diunduh saat runtime, sehingga lisensi paten Cisco berlaku) |
   | HEVC | NVENC / QSV / AMF | — (tidak ada encoder LGPL; tampilkan opsi hanya bila HW tersedia) |
   | AV1 | NVENC (RTX 40+) / QSV (Arc) / AMF (RDNA3+) | **SVT-AV1** (BSD-3 + paten AOM) |
   | VP9 | — | libvpx (BSD) |
   | ProRes / DNxHR | — | encoder native FFmpeg (`prores_ks`, `dnxhd`) |
   | Audio | — | AAC native FFmpeg, Opus (`libopus`, BSD), FLAC, MP3 via `libmp3lame` (LGPL) |
4. **Dilarang:** `libx264`, `libx265`, `libfdk_aac` (non-free), `libvidstab` (GPL-2.0), dan semua filter yang mensyaratkan `--enable-gpl`.
   Stabilisasi memakai filter `deshake` bawaan (lisensinya diverifikasi di Spike S2) atau implementasi sendiri (optical flow + smoothing jalur kamera).
5. Kepatuhan LGPL: DLL terpisah & dapat diganti, teks lisensi + tautan source + skrip build di `THIRD_PARTY_NOTICES` dan di installer.
6. **Deteksi kemampuan** saat start: probe encoder HW (di thread latar, bukan UI thread) → UI hanya menampilkan opsi yang benar-benar tersedia.

## Konsekuensi
- ➕ Patuh lisensi permisif; hasil ekspor HW cepat.
- ➖ Mesin tanpa GPU modern tidak punya ekspor HEVC; kualitas H.264 software (OpenH264) di bawah x264 → dikompensasi bitrate lebih tinggi.
