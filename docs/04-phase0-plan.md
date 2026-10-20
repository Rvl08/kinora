# 04 — Rencana Eksekusi Fase 0: Fondasi & De-risking

> Durasi target: **3 minggu** · Keluaran: keputusan go/no-go berbasis data + fondasi repo yang siap untuk Fase 1.
> Kode spike bersifat **prototipe sekali pakai** (`spikes/`), tidak dipindahkan ke kode produksi tanpa ditulis ulang.

## 1. Tujuan & definisi selesai

Fase 0 selesai bila:
1. Empat spike (S1–S4) punya **laporan terukur** dan status Lulus/Gagal terhadap kriteria di bawah.
2. Repo `kinora` berdiri dengan CI hijau (lint, test kosong, pemeriksa lisensi, build installer "hello world" yang ditandatangani).
3. ADR-0003 berubah dari *Proposed* menjadi *Accepted* (Opsi A, B, atau C).
4. Backlog Fase 1 tersusun (epic → story) dengan estimasi yang dikalibrasi ulang dari hasil spike.

## 2. Hasil audit lingkungan dev (19 Okt 2026)

| Item | Ditemukan | Status | Tindakan |
|---|---|---|---|
| Windows | 11 Pro build 26300 | ✅ | Windows Sandbox tersedia (Pro) untuk uji installer bersih |
| Rust | 1.99.1 stable MSVC | ✅ | Pin di `rust-toolchain.toml` |
| MSVC | Visual Studio Community 2026 (18.10) + SDK 10.0.26100 | ✅ | — |
| Node.js / npm | 24.21 / 11.19 | ✅ | — |
| pnpm | tidak ada | ⚠️ | `corepack enable` (bawaan Node) |
| GitHub CLI `gh` | tidak ada | ⚠️ | Opsional; dibutuhkan untuk otomatisasi rilis |
| CMake | 4.5.0-rc1 | ⚠️ | Versi RC; whisper.cpp mungkin perlu CMake stabil → pin 3.31/4.x stabil bila build gagal |
| LLVM / libclang | `D:\LLVM\bin` | ✅ | Untuk bindgen |
| FFmpeg | `FFMPEG_DIR` global → build **GPL** | ✅ diatasi (P0-05) | Repo memaksa `FFMPEG_DIR=vendor/ffmpeg-lgpl` (`.cargo/config.toml`, `force = true`); variabel global tidak diubah (Concat masih memakainya) |
| NSIS | tidak ada | ✅ | Tauri mengunduh NSIS otomatis saat bundling |
| WebView2 | 155.0.4283.62 | ✅ | WebCodecs + WebGPU tersedia |
| GPU | NVIDIA RTX 5070 Ti (driver 32.0.16.1742) | ✅ | NVENC/NVDEC (H.264, HEVC, AV1) |
| CPU / RAM | i7-14700F (20C/28T), 32 GB | ✅ | ⚠️ seri **F tanpa iGPU** → **perlu perangkat uji kedua** (laptop Intel iGPU) dan idealnya AMD |

> Catatan: `Win32_VideoController` melaporkan VRAM 4 GB karena batas 32-bit; kapasitas sebenarnya dibaca lewat DXGI saat spike.

## 3. Backlog Fase 0

| ID | Tugas | Est. | Bergantung | Kriteria penerimaan |
|---|---|---|---|---|
| P0-01 | Cek nama "Kinora": merek (PDKI/DJKI, USPTO, EUIPO), domain, handle GitHub/sosmed; siapkan 3 cadangan | 1 h | — | Nama final atau shortlist + temuan konflik |
| P0-02 | Buat repo `kinora` (monorepo sesuai 03 §5.5), `LICENSE-MIT`, `LICENSE-APACHE`, `README`, `CONTRIBUTING` (DCO), template PR dengan pernyataan clean-room | 0,5 h | P0-01 | Repo publik/privat siap; struktur folder sesuai |
| P0-03 | Toolchain pin: `rust-toolchain.toml`, `.nvmrc`, `corepack` pnpm, `.editorconfig`, rustfmt/clippy, Biome/ESLint+Prettier | 0,5 h | P0-02 | `cargo fmt/clippy` & lint JS lulus di repo kosong |
| P0-04 | CI GitHub Actions `windows-latest`: build, test, `cargo-deny`, pemeriksa lisensi JS | 1 h | P0-03 | CI hijau pada PR kosong |
| P0-05 | **FFmpeg 8.1 LGPL shared** untuk dev: build terpin + skrip fetch/verifikasi lisensi + job CI | 1 h | P0-04 | `ffmpeg -buildconf` tanpa `--enable-gpl`; ukuran DLL tercatat |
| P0-05b | Build FFmpeg **rilis** milik Kinora di CI: komponen dari [`scripts/ffmpeg/release-components.json`](../scripts/ffmpeg/release-components.json) (hasil S2), tanpa `libzvbi` dan tanpa OpenH264 (dimuat dari DLL Cisco oleh Rust), di-mirror ke GitHub Release proyek | 2 h | S2 ✅, remote GitHub | Lulus `pnpm run ffmpeg:verify:ship`; DLL terkompresi ≤ 30 MB |
| P0-06 | Pengadaan code signing (Azure Trusted Signing: verifikasi identitas/organisasi) | 1–5 h kerja (menunggu verifikasi) | — | Dapat menandatangani biner uji |
| P0-07 | **Spike S1** — pipeline preview | 5 h | P0-05 | Laporan S1 (§4) |
| P0-08 | **Spike S2** — media & encode HW | 3 h | P0-05 | Laporan S2 |
| P0-09 | **Spike S3** — runtime AI | 4 h | P0-03 | Laporan S3 |
| P0-10 | **Spike S4** — installer & signing | 2 h | P0-04, P0-06 | Laporan S4 |
| P0-11 | Fixture media uji (§6) + skrip unduh/bangkitkan | 0,5 h | P0-02 | Fixture berlisensi bebas, hash tercatat |
| P0-12 | Kontrak IPC: pilih `specta`/`ts-rs` untuk tipe Rust→TS; draf skema `project.json` v0 | 1 h | P0-03 | Tipe TS dibangkitkan otomatis di CI |
| P0-13 | Rapat **Gate go/no-go** + kalibrasi estimasi + backlog Fase 1 | 0,5 h | P0-07..10 | Notulen + ADR-0003 Accepted |

### 3.1 Progres

| ID | Status | Catatan |
|---|---|---|
| P0-01 | ⏳ Ditunda | Atas permintaan; nama kerja "Kinora" dipakai sementara. Field `repository` di `Cargo.toml` sengaja dikosongkan sampai selesai. |
| P0-02 | ✅ Selesai (19 Okt 2026) | Repo lokal `kinora/` (git `main`, belum ada commit), struktur monorepo, `LICENSE-MIT` + `LICENSE-APACHE` (teks resmi apache.org, SHA-256 `cfc7749b…3d30`), README, CONTRIBUTING (DCO + aturan clean-room), SECURITY, template PR & issue. Belum ada remote GitHub. |
| P0-03 | ✅ Selesai (19 Okt 2026) | Rust 1.99.1 dipin (`rust-toolchain.toml`), workspace edition 2024 + lint workspace, `rustfmt.toml`, `.cargo/config.toml` memaksa `FFMPEG_DIR=vendor/ffmpeg-lgpl`. JS: pnpm 12.14.0 (Corepack), ESLint 10 + typescript-eslint + eslint-plugin-svelte, Prettier 3 + plugin Svelte, TypeScript **6.0.3** (typescript-eslint belum mendukung TS 7). Semua pemeriksaan lulus: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `pnpm lint`, `pnpm format:check`. Prettier sengaja tidak memformat `*.md`. |
| P0-04 | 🟡 Selesai lokal (19 Okt 2026) | `.github/workflows/ci.yml` berisi 5 job: `rust` (Windows: fmt, clippy `-D warnings`, test, `--locked`), `web` (lint, format, lisensi JS), `cargo-deny` (lisensi/bans/sources memblokir; advisories tidak memblokir PR), `dco` (setiap commit PR wajib `Signed-off-by`), `workflows` (actionlint). Semua action dipin ke SHA commit; Dependabot memperbarui action, cargo, npm. Diverifikasi lokal: actionlint 1.7.12 bersih, pemeriksa lisensi JS (129 paket OK + uji negatif GPL/NC ditolak), logika DCO diuji di repo sementara. **Kriteria "CI hijau di GitHub" belum terbukti** karena repo belum punya remote. |
| P0-05 | ✅ Selesai (19 Okt 2026) | **FFmpeg 8.1.3**, bukan 7.x (BtbN tidak lagi menerbitkan 7.x; semua binding Rust mendukung 8.1) — ADR-0005 Amandemen 1. `scripts/ffmpeg/lock.json` (SHA-256 `638dd2a0…ec9a1`), `pnpm run ffmpeg:fetch` (unduh + verifikasi hash + ekstrak ke `vendor/ffmpeg-lgpl`, idempoten), `pnpm run ffmpeg:verify` (dev) / `ffmpeg:verify:ship` (rilis). Hasil: tanpa `--enable-gpl`/`--enable-nonfree`, **LGPL v3** (`--enable-version3`), 7 DLL = 159,1 MB mentah / ±66 MB zip, encoder 18/18, decoder 6/6, hwaccel 5/5, filter 5/5. Mode `--ship` **gagal dengan sengaja**: `libzvbi` (GPL-2.0+) dan OpenH264 statis (tanpa lisensi paten Cisco) → diselesaikan di P0-05b. Job CI `ffmpeg` (Windows, cache arsip) ditambahkan. |
| P0-07 (S1) | ✅ Selesai untuk GPU diskrit (19 Okt 2026) | Opsi A terbukti: 1080p × 3 + efek 30 fps drop 0%, 4K60 HEVC10 59,96 fps, drift 10 menit 0 frame terlambat, seek proxy p95 62 ms, PSNR 96–97 dB / 67–68 dB. Ditemukan & diperbaiki: kebocoran memori `Packet::read` (ffmpeg-next) dan seleksi frame VFR. **Belum:** validasi iGPU (tidak ada perangkat), 1 kegagalan seek VFR intermiten dari 180. Laporan: [`spikes/S1-report.md`](spikes/S1-report.md). |
| P0-08 (S2) | ✅ Selesai untuk NVIDIA (20 Okt 2026) | Ekspor 1080p NVENC 12,2× realtime (target ≥ 2×), 4K HEVC 1,2–2,3× (target ≥ 1×), ekspor penuh + mix audio 12,8×; zero-copy D3D11→NVENC (CPU 18–57%); composite wgpu −2% (4K) / −20% (1080p); VMAF grafis 96–98 @6 Mbps. OpenH264 via DLL Cisco saat runtime (crate `openh264`) → FFmpeg rilis tanpa OpenH264. Daftar komponen P0-05b tervalidasi (133 nama). **Gagal:** ukuran DLL dev 66 MB (> 60) → P0-05b. **Belum:** QSV, AMF, decode CUDA via API. Laporan: [`spikes/S2-report.md`](spikes/S2-report.md). |

*(h = hari kerja satu developer.)* Jalur kritis: P0-02 → P0-03 → P0-05 → S1 → Gate. S3 dan S4 berjalan paralel.

## 4. Spesifikasi spike

### S1 — Pipeline preview (ADR-0003)
- **Hipotesis:** demux Rust → WebCodecs → WebGPU memberi preview real-time tanpa menyalin frame mentah lewat IPC.
- **Prototipe:** jendela Tauri + Svelte minimal; satu `<canvas>` WebGPU; 3 layer video + 1 teks; 2 efek WGSL (color adjust, gaussian blur); playback, pause, seek, scrub; audio playback dari Rust `cpal` sebagai clock.
- **Ukur:** fps (rata-rata, p1 low), frame drop %, latensi seek (p50/p95), drift A/V setelah 10 menit, CPU %, RAM proses (WebView2 + host), bandwidth IPC, PSNR preview vs render wgpu offline untuk frame yang sama.
- **Matriks uji:** {RTX 5070 Ti, laptop Intel iGPU, (opsional) AMD} × {1080p30 H.264, 4K60 HEVC 10-bit, 4K ProRes→proxy, VFR dari HP}.
- **Lulus:** kriteria ADR-0003 terpenuhi di RTX dan iGPU. **Gagal sebagian:** iGPU < 30 fps → coba Opsi B selama maks. 3 hari, lalu bandingkan.

### S2 — Media & encode
- **Hipotesis:** FFmpeg LGPL + encoder HW cukup untuk ekspor cepat tanpa komponen GPL.
- **Prototipe:** CLI Rust: probe → decode HW (D3D11VA/NVDEC) → (pass-through wgpu) → encode NVENC/QSV/AMF/OpenH264/SVT-AV1 → mux MP4; mixer audio sederhana; pembuatan proxy & thumbnail strip.
- **Ukur:** kecepatan ekspor (× realtime) per encoder, kualitas (VMAF/PSNR pada bitrate sama), ukuran total DLL FFmpeg, waktu probe encoder saat start, kebenaran warna (BT.709/BT.2020, full/limited range).
- **Verifikasi lisensi:** `pnpm run ffmpeg:verify` (sudah ada sejak P0-05). Tambahan S2: buktikan OpenH264 dapat dimuat dari DLL resmi Cisco saat runtime (delay-load/`LoadLibrary`) tanpa membuat `avcodec` gagal dimuat bila DLL itu belum ada.
- **Masukan untuk P0-05b:** daftar codec/format/filter yang benar-benar dipakai → konfigurasi build rilis yang dipangkas.
- **Lulus:** 1080p H.264 HW ≥ 2× realtime; 4K HEVC HW ≥ 1× realtime; DLL FFmpeg ≤ 60 MB terkompresi.

### S3 — Runtime AI lokal
- **Hipotesis:** ORT + DirectML dan whisper.cpp berjalan di GPU tanpa instalasi tambahan (CUDA/Python tidak dibutuhkan).
- **Prototipe:** (a) BiRefNet-lite cutout 1080p via ORT DirectML; (b) whisper.cpp base & large-v3-turbo (backend Vulkan, fallback CPU) dengan *word timestamps* pada audio Indonesia & Inggris; (c) Kokoro via sherpa-onnx **dengan dan tanpa espeak-ng** (05 §4); (d) Silero VAD; (e) unduhan model + verifikasi SHA-256 + resume.
- **Ukur:** ms/frame cutout, RTF transkripsi, WER kasar pada 3 klip Indonesia, akurasi timestamp kata (±80 ms), RAM/VRAM puncak, ukuran DLL runtime (ORT + DirectML).
- **Lulus:** semua jalan di mesin bersih (Windows Sandbox) hanya dengan file dari installer; caption 10 menit (base, GPU) ≤ 60 dtk; sherpa-onnx dapat dipakai tanpa espeak-ng (atau alternatif G2P teridentifikasi).

### S4 — Installer, signing, update
- **Hipotesis:** Tauri NSIS menghasilkan `setup.exe` kecil, per-user, tanpa prasyarat, tidak diblokir SmartScreen.
- **Prototipe:** app "hello" + FFmpeg LGPL DLL + ORT DirectML DLL; NSIS dengan wizard (bahasa ID/EN, lisensi, lokasi, shortcut), WebView2 `embedBootstrapper`, asosiasi `.kinora`, uninstaller (opsi hapus model/cache); updater Tauri dengan kunci tanda tangan; signing di CI.
- **Uji:** pasang di Windows Sandbox bersih (Win11) dan VM Windows 10 22H2 tanpa WebView2; upgrade dari versi lama; uninstall bersih.
- **Ukur:** ukuran installer, waktu instal, ukuran terpasang, reaksi SmartScreen/Defender.
- **Lulus:** ≤ 120 MB; instal < 30 dtk; aplikasi terbuka di mesin bersih; tidak ada peringatan "unknown publisher".

## 5. Jadwal (3 minggu)

| Minggu | Fokus |
|---|---|
| 1 | P0-01..06, P0-11, P0-12 · mulai S3 & S4 (tidak bergantung FFmpeg) |
| 2 | S1 & S2 (setelah FFmpeg LGPL siap) · lanjut S3 & S4 |
| 3 | Uji matriks perangkat · laporan spike · Gate P0-13 · backlog Fase 1 |

## 6. Fixture media uji
Semua berlisensi bebas (CC0/CC-BY atau rekaman sendiri), disimpan di luar git (diunduh via skrip + hash):
1080p30 H.264 (HP, VFR) · 4K60 HEVC 10-bit HDR · 4K ProRes 422 · 1080p AV1 · WebM VP9 dengan alpha · foto HEIC & JPEG ·
audio: podcast Indonesia (banyak "eee/anu"), wawancara 2 pembicara, lagu dengan vokal · klip layar (screen recording) untuk teks tajam.

## 7. Risiko khusus Fase 0
| Risiko | Mitigasi |
|---|---|
| Tidak ada perangkat iGPU untuk uji | Pinjam/sewa laptop Intel iGPU; minimal uji via `Microsoft Basic Render Driver`/WARP sebagai batas bawah |
| Verifikasi Azure Trusted Signing lama | Mulai P0-06 di hari pertama; fallback sertifikat OV komersial |
| CMake RC memecah build whisper.cpp | Pasang CMake stabil berdampingan |
| `FFMPEG_DIR` global (GPL) terpakai tanpa sengaja | Set `[env] FFMPEG_DIR` di `.cargo/config.toml` repo Kinora + cek `-buildconf` di CI |

## 8. Keluaran untuk Gate
`docs/spikes/S1-report.md` … `S4-report.md` (angka mentah + grafik + rekomendasi), ADR-0003 final, estimasi Fase 1 yang dikalibrasi, backlog Fase 1.
