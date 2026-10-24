# Laporan P0-05b — Build FFmpeg Rilis Statis (LGPL)

> **Status:** ✅ **Terverifikasi di GitHub Actions** (24 Okt 2026, run [40068909424](https://github.com/Rvl08/kinora/actions/runs/40068909424), commit `ce84f83`, PR #4).
> Build statis, gerbang rilis, dan probe Rust semuanya lulus pada `windows-2025`. Verifikasi pindah dari Windows Sandbox ke CI karena sejak 24 Okt Smart App Control juga **aktif di dalam Sandbox**.
> **ADR terkait:** [ADR-0005](../adr/0005-codec-and-ffmpeg-policy.md) Amandemen 2–4, [ADR-0006](../adr/0006-local-ai-runtime.md) Amandemen 1, [ADR-0007](../adr/0007-code-signing.md) Amandemen 1
> **Kode:** [`scripts/ffmpeg/release/`](../../scripts/ffmpeg/release/) · [`spikes/ffmpeg-static-probe/`](../../spikes/ffmpeg-static-probe/) · [`.github/workflows/ffmpeg-release.yml`](../../.github/workflows/ffmpeg-release.yml)

## 1. Ringkasan

| Kriteria | Target | Hasil (run 40068909424) | Status |
|---|---|---|---|
| Sumber dipin | Versi, URL, SHA-256, lisensi; tanpa GPL | 10 sumber + 4 alat + 8 paket MSYS2 di [`release-sources.json`](../../scripts/ffmpeg/release-sources.json), semua diverifikasi SHA-256 saat dipakai. LAME punya mirror Debian yang byte-identik | ✅ |
| Build statis `.lib` + header | Komponen = `release-components.json`, CRT `/MT` | 13 library statis, 60,2 MB (FFmpeg sendiri 27,8 MB), header 3,4 MB. Build + gerbang **10,6 menit** (cache unduhan hangat) | ✅ |
| Gerbang rilis | Tanpa gpl/nonfree, LGPL, tanpa zvbi/openh264, komponen persis, CRT, tanpa `/GL` | `FFmpeg licence check (release): OK`. Komponen persis sesuai daftar (decoder 39, termasuk 1 *implicit*; encoder 25; parser 22; bsf 10, termasuk 2 *implicit*; hwaccel 28; demuxer 20; muxer 16; protocol 2; filter 24). Tidak ada objek yang meminta CRT DLL | ✅ |
| Probe Rust statis | Tertaut statis, codec bekerja, tanpa DLL FFmpeg/CRT | **9/9 uji bolak-balik lulus** (§3). `dumpbin /dependents`: hanya DLL sistem Windows (kernel32, ntdll, ole32, user32, advapi32, bcrypt, api-ms-win-core-synch) | ✅ |
| Ukuran yang dikirim | (dahulu: DLL terkompresi ≤ 30 MB) | Tambahan FFmpeg pada exe: **20,66 MB** (exe probe 20,89 MB vs baseline 0,23 MB), belum dikompresi. Ukuran terkompresi di dalam installer diukur di S4 | ✅ / ⏳ S4 |
| Workflow CI | Dispatch + path, cache, SHA pin, izin minimal, batas waktu | Lulus `actionlint` (job CI). Langkah build dibatasi 90 menit, dan kegagalan dilaporkan sebagai *annotation* publik | ✅ |
| Kepatuhan LGPL | `RELINKING.md`, notices, sumber lengkap | Ditulis. Artefak (72,5 MB) membawa semua arsip sumber. Build mendukung `--source ffmpeg=<dir>` | ✅ |

## 2. Metode

- **Satu skrip untuk CI dan lokal:** [`release/build.mjs`](../../scripts/ffmpeg/release/build.mjs) (Node).
  - Unduhan memakai `curl.exe` bawaan Windows (bertanda tangan Microsoft) dan diverifikasi SHA-256, lalu diekstrak ke folder kosong.
  - Dependensi dibangun dengan CMake/Meson (Ninja). libvpx memakai configure + MSBuild (`WholeProgramOptimization=false`); FFmpeg memakai `configure --toolchain=msvc` di bash MSYS2.
  - Flag configure dibangkitkan dari `release-components.json` lewat [`components.mjs`](../../scripts/ffmpeg/release/components.mjs): `--disable-everything --disable-autodetect`, lalu setiap komponen disebut eksplisit.
- **CRT:** `/MT` (LIBCMT) di semua library; Rust memakai `-C target-feature=+crt-static` (ADR-0006 A1).
- **Gerbang tanpa eksekusi:** [`release/coff.mjs`](../../scripts/ffmpeg/release/coff.mjs) membaca arsip COFF, sehingga gerbang juga bisa berjalan di mesin dengan Smart App Control. Yang diperiksa:
  - konfigurasi dan lisensi dari string di library;
  - komponen dari referensi tak terdefinisi `all*.o`;
  - simbol terlarang (`Wels*`, `vbi_*`, `x264_*`, …);
  - `/DEFAULTLIB`;
  - objek `/GL`.
- **Lisensi FFmpeg:** tanpa `--enable-version3` → **LGPL-2.1-or-later**.
- **Pelaporan kegagalan:** log job GitHub hanya bisa dibaca pengguna yang login. Karena itu [`report-failure.mjs`](../../scripts/ffmpeg/release/report-failure.mjs) menerbitkan 180 baris terakhir log build/probe sebagai *annotation*, yang bisa dibaca lewat API tanpa kredensial. Bila configure gagal, `build.mjs` mencetak ekor `ffbuild/config.log`.

## 3. Hasil probe (link statis)

Run akhir 40068909424 (`ce84f83`) lulus. Probe keluar dengan kode 0 hanya bila **semua** uji lulus, tetapi keluaran per uji dari run sukses tidak dipublikasikan, karena *annotation* hanya terbit saat gagal dan log job butuh login. Angka di bawah berasal dari run 40067496890 (`5678e58`), yang kodenya identik kecuali penanganan paket akhir FLAC.

| Uji | Encoder → muxer → demuxer → decoder | Hasil |
|---|---|---|
| AV1 | libsvtav1 → Matroska → libdav1d | 24 frame, galat luma maks 6,07 |
| VP9 | libvpx-vp9 → WebM → vp9 | 24 frame, galat 0,82 |
| ProRes | prores_ks → MOV → prores | 24 frame, galat 0,01 |
| MJPEG | mjpeg → MP4 → mjpeg | 24 frame, galat 0,36 |
| Opus | libopus → Ogg → opus | RMS 0,353 (target 0,354) |
| MP3 | libmp3lame → mp3 → mp3float | RMS 0,336 |
| AAC | aac → M4A → aac | RMS 0,353 |
| FLAC | flac → flac → flac | gagal di 40067496890 pada paket akhir tanpa data; lulus di run akhir setelah paket itu dilewati (temuan 9) |
| PCM | pcm_s16le → WAV → pcm_s16le | RMS 0,354 |
| Filter + swscale | `scale,hflip,pad` → 176×128; swscale → 160×120 RGB24 | lulus |

Probe juga mendaftar 64 codec dan tipe perangkat HW: cuda, dxva2, qsv, d3d11va, d3d12va, amf.

## 4. Temuan

1. **`release-components.json` dari S2 tidak cukup untuk build `--disable-everything`.** Ditambahkan:
   - `parsers` (22);
   - `hwaccels` (28);
   - `hwdevices` dan `platform`: tanpa `w32threads`, FFmpeg berjalan single-thread;
   - `implicit`: decoder `h263` (dipilih decoder mpeg4), bsf `pgs_frame_merge` dan `vp9_superframe_split`.
2. **Nama khusus configure:**
   - filter `buffer`/`buffersink`/`abuffer`/`abuffersink` bawaan, tidak boleh disebut ke configure;
   - demuxer `mpeg` = `mpegps`;
   - codec **`mov_text` = komponen `movtext`**. Sebelumnya codec ini diam-diam tidak terbangun dan baru tertangkap oleh gerbang.
3. **`--extra-ldflags` untuk MSVC harus memakai sintaks `-libpath:`.** configure hanya menerjemahkan `-L` yang datang dari pkg-config, sehingga `-L` biasa diabaikan `link.exe` (LNK4044). Akibatnya pemeriksaan `check_lib` untuk LAME gagal. Konsekuensi lanjutannya, `-libpath:` ikut masuk `EXTRALIBS`, jadi pengurai pustaka harus hanya menerima nama (`-lNAME`, `NAME.lib`).
4. **File `.pc` dari CMake harus dinormalkan:**
   - setiap `-lNAME` harus ada sebagai `NAME.lib`;
   - `vpl.pc` statis butuh `advapi32 ole32 shlwapi uuid`.
5. **Proyek MSBuild libvpx mengaktifkan `/GL` di Release.** Objek `/GL` adalah IR compiler, bukan COFF: tidak bisa diperiksa gerbang dan hanya bisa ditautkan oleh MSVC versi yang sama. Setelah dimatikan, `vpx.lib` turun dari 22,3 MB menjadi 10,8 MB.
6. **Undici (`fetch` Node) bisa crash dengan *assertion* internal** saat mengikuti rantai redirect SourceForge, dan crash ini mematikan seluruh proses. Unduhan kini memakai `curl.exe` sistem sebagai proses terpisah.
7. **nv-codec-headers dipin di 12.2** (driver NVIDIA ≥ 551.76; cakupan perangkat lebih luas daripada 13.x yang butuh ≥ 570). Encode 4:2:2 Blackwell tidak tersedia.
8. **`ffmpeg-next` punya dua jebakan yang relevan untuk exporter Fase 1:**
   - `data_mut(c)` untuk audio *planar* memakai `linesize[c]`, padahal FFmpeg hanya mengisi `linesize[0]`, sehingga bidang ke-2 kosong;
   - `Source::add` (buffersrc) mengambil alih dan mengosongkan frame;
   - selain itu, WAV tanpa *channel mask* menghasilkan layout *unspecified* yang ditolak `swr_convert_frame` ("Input changed").
9. **FLAC mengakhiri stream dengan paket tanpa data** yang hanya membawa STREAMINFO baru (`AV_PKT_DATA_NEW_EXTRADATA`). Jalur mux probe menolaknya ("Invalid data"). Probe melewatinya karena yang dibuktikan adalah codec dan tautan. **Exporter Fase 1 wajib menangani pembaruan STREAMINFO ini** (setara CLI `ffmpeg`, yang menulis ulang header di akhir), dan penyebab pastinya perlu diselidiki saat itu.
10. **Lingkungan:** Smart App Control aktif di Windows Sandbox (24 Okt), sehingga `bash.exe` MSYS2 dan `cargo.exe` diblokir di sana. `rustfmt` toolchain 1.99.1 di host juga mulai diblokir. Semua verifikasi build Windows kini lewat CI.

### Jejak iterasi CI (PR #4, 24 Okt 2026)

| Run | Gagal di | Perbaikan |
|---|---|---|
| f4ff35c | Tidak terbaca (log butuh login) | Kegagalan diterbitkan sebagai *annotation* (`14f539d`) |
| 14f539d | Undici crash pada unduhan LAME | `curl.exe` + mirror Debian (`bee843e`) |
| bee843e | configure: libvpl tidak ditemukan | Normalisasi `.pc` + ekor `config.log` (`59d95ff`) |
| 59d95ff | configure: LAME (`-L` diabaikan) | `-libpath:` (`fb91dd4`) |
| fb91dd4 | Gerbang: `/GL` di vpx, `mov_text`, *implicit* | `1a7504b` |
| 1a7504b | Probe: pustaka palsu `ibpath` | Pengurai `EXTRALIBS` (`bb2ecd4`) |
| bb2ecd4 | Probe: audio planar LAME | Pointer bidang (`f1b9eec`) |
| f1b9eec | Probe: FLAC, PCM, filter | `5678e58` |
| 5678e58 | Probe: paket akhir FLAC | `ce84f83` |
| **ce84f83** | — | **Lulus semua** |

## 5. Belum dibuktikan / tindak lanjut

- **Ukuran terkompresi di installer:** diukur di S4 bersama exe Kinora sungguhan.
- **Publikasi artefak:** jalankan workflow dengan `publish: true` (manual, setelah PR digabung). Release membawa zip, laporan gerbang, bukti probe, dan semua arsip sumber.
- **Exporter Fase 1:** temuan 8 dan 9.
- **QSV/AMF:** hwaccel dan hwdevice terdaftar (probe menemukan `qsv` dan `amf`) tetapi belum diuji di perangkat Intel/AMD (sama dengan S2).
