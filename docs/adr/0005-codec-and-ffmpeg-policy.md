# ADR-0005 — Kebijakan codec & build FFmpeg

- **Status:** Accepted (19 Okt 2026), **diamandemen** 19 Okt 2026 (Amandemen 1, P0-05) dan 20 Okt 2026 (Amandemen 2, Spike S2) · **Keputusan terkait:** D2

## Konteks
Lisensi proyek permisif (ADR-0002) melarang komponen GPL. FFmpeg berlisensi LGPL-2.1+ bila dibangun tanpa
`--enable-gpl`/`--enable-nonfree`. Paten H.264/HEVC memengaruhi encoder software.

> ⚠️ Catatan lingkungan dev: `FFMPEG_DIR` di mesin saat ini = `D:\Ffmpeg\ffmpeg-master-latest-win64-gpl-shared` (**GPL**, untuk Concat).
> Kinora wajib memakai build **LGPL** terpisah dan menyetel `FFMPEG_DIR` per-proyek (mis. `.cargo/config.toml` `[env]`), bukan global.

## Keputusan
1. **FFmpeg 8.1 LGPL shared** (DLL dinamis; semula 7.x, lihat Amandemen 1), dari build reproducible milik proyek (CI) — basis konfigurasi BtbN `lgpl-shared`, dipangkas ke codec yang dibutuhkan.
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

## Amandemen 1 — hasil P0-05 (19 Okt 2026)

**Temuan (observasi):**
1. BtbN tidak lagi menerbitkan build FFmpeg 7.x; yang tersedia 8.1 (8.1.3), 9.0 (9.0.2), dan master.
2. Dukungan binding Rust per 19 Okt 2026: `ffmpeg-next` ≤ 9.0, `ffmpeg-the-third` ≤ 9.0, `rusty_ffmpeg` ≤ 8.1, `rsmpeg` ≤ 8.0.
3. Build BtbN `win64-lgpl-shared-8.1` (n8.1.3-18-g46d899d7a0) dikonfigurasi `--enable-version3` → lisensi efektif **LGPL-3.0-or-later**, bukan 2.1.
   Ini tetap kompatibel dengan distribusi Kinora (MIT OR Apache-2.0) selama FFmpeg tetap DLL terpisah yang dapat diganti pengguna.
4. Build itu membawa 44 library eksternal, termasuk dua yang **belum aman dikirim**:
   - `libzvbi`: upstream menyatakan GPL-2.0-or-later kecuali file tertentu, walau `configure` FFmpeg 8.1 tidak menandainya GPL. Kinora memperlakukannya sebagai GPL.
   - `libopenh264` dibangun statis dari source: lisensi paten Cisco hanya berlaku untuk biner buatan Cisco.
5. Ukuran: 7 DLL = 159,1 MB mentah, ±66 MB terkompresi zip. `avcodec` sendiri 86,7 MB. Ini terlalu besar untuk anggaran installer ≤ 120 MB.
6. Semua kemampuan yang dibutuhkan ADR ini tersedia: 18/18 encoder, 6/6 decoder, 5/5 hwaccel (d3d11va, d3d12va, dxva2, cuda, qsv), filter `deshake`/`loudnorm`/`ebur128`/`scale`/`zscale`.

**Keputusan:**
1. **Versi: FFmpeg 8.1.x** (bukan 7.x). Alasan: cabang stabil yang sudah matang, didukung semua keluarga binding, dan masih menerima rilis titik. Naik ke 9.x dievaluasi setelah Spike S2.
2. **Dua jalur build:**
   - **Dev/spike:** build BtbN yang dipin di `scripts/ffmpeg/lock.json` (SHA-256 diverifikasi), dipasang dengan `pnpm run ffmpeg:fetch`. Tidak pernah didistribusikan.
   - **Rilis:** build Kinora sendiri (**P0-05b**), dipangkas ke codec yang terbukti dibutuhkan oleh Spike S2, **tanpa `libzvbi`**, dan OpenH264 dimuat dari DLL Cisco saat runtime (atau dihapus). Build ini wajib lulus `pnpm run ffmpeg:verify:ship`.
3. Lisensi FFmpeg dicatat sebagai **LGPL-3.0-or-later** di `THIRD_PARTY_NOTICES` selama `--enable-version3` aktif.

## Amandemen 2 — hasil Spike S2 (20 Okt 2026)

Dasar: [S2-report.md](../spikes/S2-report.md).

1. **H.264 software fallback = OpenH264 dari DLL resmi Cisco, dimuat saat runtime oleh crate Rust `openh264` (`libloading`)**, **bukan** lewat `--enable-libopenh264`. FFmpeg rilis tidak menautkan OpenH264 sama sekali.
   DLL diunduh saat dibutuhkan dan diverifikasi tiga lapis: MD5 Cisco, SHA-256 allow-list, Authenticode (status valid + timestamp).
2. **Jalur ekspor:** decode D3D11VA → composite wgpu bila ada efek → encoder hardware; tanpa efek, **zero-copy** D3D11 → encoder. Decode software hanya sebagai fallback.
3. **Encoder default:** H.264 hardware; HEVC/AV1 hardware opsional, 10-bit dipertahankan. Fallback tanpa GPU: SVT-AV1 (AV1) dan OpenH264-Cisco (H.264, mode kualitas). VP9 hanya untuk ekspor WebM eksplisit (0,5× realtime di 1080p).
4. **Wajib di exporter:** reset `pict_type`/flag key setiap frame; time base encoder = 1/fps dengan rescale timestamp **dan** durasi; `av_frame_ref`, bukan deep copy; `get_format` eksplisit; VBV hanya untuk encoder hardware.
5. **Build rilis (P0-05b):** komponen sesuai [`scripts/ffmpeg/release-components.json`](../../scripts/ffmpeg/release-components.json), divalidasi `scripts/ffmpeg/check-components.py`; tanpa protokol jaringan, tanpa filter berat (efek dikerjakan compositor WGSL).
6. **Belum teruji:** QSV & AMF (tidak ada perangkat), decode CUDA lewat API. Keduanya wajib diuji sebelum rilis; tabel encoder §3 tetap berlaku sebagai kebijakan.
