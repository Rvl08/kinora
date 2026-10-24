# Laporan P0-05b — Build FFmpeg Rilis Statis (LGPL)

> **Status:** ⏸️ **Terhenti, belum terverifikasi** (24 Okt 2026). Skrip build, gerbang rilis, workflow CI, probe Rust, dan dokumen LGPL sudah ditulis. Build penuh belum pernah selesai: sejak 24 Okt Smart App Control juga **aktif di Windows Sandbox**, sehingga sandbox tidak bisa lagi menjalankan toolchain yang tidak bertanda tangan.
> **ADR terkait:** [ADR-0005](../adr/0005-codec-and-ffmpeg-policy.md) Amandemen 2–3, [ADR-0006](../adr/0006-local-ai-runtime.md) Amandemen 1, [ADR-0007](../adr/0007-code-signing.md) Amandemen 1
> **Kode:** [`scripts/ffmpeg/release/`](../../scripts/ffmpeg/release/) · [`spikes/ffmpeg-static-probe/`](../../spikes/ffmpeg-static-probe/) · [`.github/workflows/ffmpeg-release.yml`](../../.github/workflows/ffmpeg-release.yml)

## 1. Ringkasan

| Kriteria | Target | Hasil | Status |
|---|---|---|---|
| Sumber dipin | Versi, URL, SHA-256, lisensi; tanpa GPL | 10 sumber + 4 alat + 8 paket MSYS2 di [`release-sources.json`](../../scripts/ffmpeg/release-sources.json), semua hash dihitung dari unduhan | ✅ |
| Build statis `.lib` + header | Komponen = `release-components.json`, CRT `/MT` | zlib, opus, LAME, SVT-AV1, libvpl, dav1d **berhasil dibangun** di sandbox (22 Okt). libvpx dan FFmpeg belum | ⚠️ sebagian |
| Gerbang rilis | Tanpa gpl/nonfree, LGPL, tanpa zvbi/openh264, komponen persis, ukuran | `verify.mjs --release` ditulis. Parser COFF diuji pada `libcmt.lib`/`libconcrt.lib` MSVC; belum pada artefak FFmpeg | ⚠️ belum dijalankan |
| Probe Rust statis | Konfigurasi, lisensi, codec, decode, `dumpbin` | Ditulis, `Cargo.lock` dibuat; belum dikompilasi | ❌ belum diuji |
| Workflow CI | Dispatch + path, cache, SHA pin, izin minimal | Ditulis. `actionlint` tidak tersedia di host (biner tak bertanda tangan) | ⚠️ belum di-lint |
| Kepatuhan LGPL | `RELINKING.md`, fragmen notices | Ditulis; build mendukung `--source ffmpeg=<dir>` | ✅ |

## 2. Metode (yang dibangun)

- **Satu skrip untuk CI dan lokal:** [`release/build.mjs`](../../scripts/ffmpeg/release/build.mjs) (Node).
  - Semua unduhan diverifikasi SHA-256, lalu diekstrak ke folder kosong masing-masing.
  - Lingkungan MSVC diambil dari vcvars (lewat vswhere bila belum ada).
  - Dependensi dibangun dengan CMake/Meson (Ninja). libvpx memakai configure + MSBuild, FFmpeg memakai `configure --toolchain=msvc` di bash MSYS2.
  - Paket MSYS2 diekstrak langsung (tanpa pacman/keyring).
  - Flag configure dibangkitkan dari `release-components.json` lewat [`components.mjs`](../../scripts/ffmpeg/release/components.mjs): `--disable-everything --disable-autodetect`, lalu setiap komponen dan fitur disebut eksplisit. Peringatan configure dianggap error.
- **CRT:** semua `/MT` (LIBCMT): CMake `CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded`, Meson `b_vscrt=mt`, libvpx `--enable-static-msvcrt`, FFmpeg `-MT`. Rust: `-C target-feature=+crt-static` (sesuai ADR-0006 A1). Gerbang menolak objek yang meminta MSVCRT/UCRT DLL atau objek `/GL` (LTO SVT-AV1 dimatikan).
- **Gerbang tanpa eksekusi:** [`release/coff.mjs`](../../scripts/ffmpeg/release/coff.mjs) membaca arsip COFF.
  - Konfigurasi dan lisensi diambil dari string di library.
  - Komponen terdaftar dibaca dari referensi tak terdefinisi di `allcodecs.o`, `allformats.o`, `parsers.o`, `bitstream_filters.o`, `protocols.o`, dan `allfilters.o`. Hwaccel dan hwdevice dibaca dari indeks simbol.
  - Simbol terlarang (`Wels*`, `vbi_*`, `x264_*`, …) dan arahan `/DEFAULTLIB` juga diperiksa.
  - Dengan begitu gerbang bisa berjalan di mesin dengan SAC.
- **Lisensi FFmpeg:** tanpa `--enable-version3` (tidak ada library yang membutuhkannya) → **LGPL-2.1-or-later**.

## 3. Temuan

1. **`release-components.json` S2 tidak cukup untuk build `--disable-everything`.** P0-05b menambahkan beberapa grup:
   - `parsers` (22): tanpa parser, mpegts/mp3/aac/flac tidak ter-demux dengan benar;
   - `hwaccels` (28: d3d11va/d3d11va2/d3d12va/dxva2/nvdec);
   - `hwdevices` dan `platform`: dengan `--disable-autodetect`, `w32threads` ikut mati, sehingga FFmpeg menjadi single-thread bila tidak disebut;
   - `implicit` (masih kosong: diisi dari build pertama).
2. **Nama khusus:** filter `buffer/buffersink/abuffer/abuffersink` bawaan dan tidak boleh disebut ke configure. Demuxer `mpeg` = `mpegps`.
3. **nv-codec-headers dipin di 12.2** (driver NVIDIA ≥ 551.76), bukan 13.x (≥ 570), agar lebih banyak pengguna mendapat NVENC. Fitur SDK 13 (mis. 4:2:2 Blackwell) tidak tersedia. Ini perlu keputusan lead.
4. **LAME (LGPL-2.0+)** diizinkan oleh ADR-0005 §3, tetapi CLAUDE.md §3 menulis "LGPL hanya untuk FFmpeg". Salah satunya perlu diselaraskan.
5. **Sandbox:**
   - pacman menolak database tanpa keyring, lalu diganti ekstraksi langsung;
   - libvpx gagal ketika `CC=cl` diset, lalu `unset CC`;
   - exe di folder ter-mapping kadang gagal start, sehingga alat diekstrak ke `C:\kft`;
   - Monitor `tail -F` di host pada `job.log` membuat transkrip hilang: jangan tail file yang sedang ditulis VM.
6. **24 Okt: SAC aktif di sandbox.** `bash.exe` MSYS2 (tidak bertanda tangan) diblokir (`spawnSync … UNKNOWN`, job 6). Jalur lokal kini tidak bisa lagi membangun.

## 4. Yang belum terbukti

- Build libvpx (MSBuild), build FFmpeg, dan gerbang rilis pada artefak nyata, termasuk daftar `implicit`.
- Probe: link statis, decode dav1d/VP9/ProRes/MJPEG, audio Opus/LAME/AAC/FLAC, `dumpbin /dependents`, ukuran exe.
- `actionlint` dan run pertama workflow di GitHub Actions.
- Ukuran (target ≤ 30 MB terkompresi) dan waktu build.

## 5. Langkah berikutnya

1. Jalankan `FFmpeg release build` (workflow_dispatch, `publish: false`) di GitHub Actions. Runner tidak terkena SAC, dan workflow memakai skrip yang sama.
2. Isi `implicit` dari laporan gerbang, perbaiki error build bila ada, lalu jalankan ulang.
3. Isi laporan ini dengan angka (ukuran `.lib`, delta exe, waktu build), lalu publikasikan dengan `publish: true`.
