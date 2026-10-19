# 03 — Master Plan: "Kinora" Desktop Video Editor

> Dokumen perencanaan. **Belum ada satu baris kode pun.** Semua pilihan teknis di sini masih
> berstatus *proposal* sampai keputusan di §9 disetujui dan spike Fase 0 lulus.
>
> Dasar: [01-reverse-engineering-report.md](01-reverse-engineering-report.md) · [02-feature-matrix.md](02-feature-matrix.md)

---

## 1. Visi & positioning

**Satu kalimat:** Editor video desktop sekelas CapCut, **100% gratis, offline, tanpa akun, tanpa watermark**,
dengan AI yang berjalan di perangkat pengguna — dan dipasang semudah *Next → Next → Finish*.

| Masalah | CapCut | Concat | Jawaban Kinora |
|---|---|---|---|
| Biaya | Fitur inti AI dikunci Pro/kredit | Gratis | Gratis, tanpa kredit/kuota |
| Instalasi | Mudah tapi ±1,6 GB | Build dari source rumit; rilis unsigned | Setup.exe ±100 MB, **signed**, tanpa prasyarat |
| Privasi | Akun + cloud | Lokal | Lokal; jaringan hanya untuk unduh model/update (bisa dimatikan) |
| Kelengkapan | Sangat lengkap | Beta, banyak celah | Paritas Concat + ±85% fitur lokal CapCut |
| Keunikan | Generatif cloud | API JSON-RPC | **Edit via transkrip, AI Copilot lokal, server MCP, Bahasa Indonesia first-class, interop NLE** |

**Persona utama**
1. **Kreator short-form** (TikTok/Reels/Shorts) — butuh caption viral, reframe 9:16, potong cepat.
2. **Edukator/podcaster** — edit berbasis transkrip, hapus "eee/anu", noise reduction, chapter.
3. **UMKM/marketer** — template, brand kit, batch ekspor multi-rasio.
4. **Power user/dev** — scripting, MCP/agent, ekspor ke DaVinci/Premiere.

## 2. Keunggulan & keunikan (pembeda yang disengaja)

1. **Transcript-first editing** — transkrip adalah "timeline kedua": hapus kalimat ⇒ klip terpotong; cari kata ⇒ lompat ke momen.
2. **Pembersih bicara 1-klik** — filler words multi-bahasa (termasuk kamus Indonesia: *eee, emm, anu, jadi, kayak*), jeda panjang, ulang kata.
3. **AI Copilot lokal** — perintah bahasa alami ("buat versi 30 detik, caption kuning, musik turun saat bicara") diterjemahkan
   menjadi *command* editor yang sama dengan UI → bisa di-undo, transparan. Mesin: Ollama/llama.cpp lokal, atau BYOK.
4. **Server MCP bawaan** — agent eksternal (Claude, dsb.) dapat membuka proyek & mengedit via protokol standar. Belum ada di CapCut maupun Concat.
5. **Long video → shorts** gratis — skoring highlight dari transkrip + energi audio + (opsional) LLM.
6. **Bahasa Indonesia kelas satu** — UI, kamus filler, TTS Indonesia (Piper), terjemahan caption offline.
7. **Hardware-aware** — deteksi GPU/NPU/RAM saat pertama jalan → profil performa, rekomendasi ukuran model, proxy otomatis.
8. **Satu shader, dua runtime** — efek WGSL yang sama untuk preview (WebGPU) dan ekspor (wgpu) ⇒ "apa yang terlihat = apa yang diekspor".
9. **Format terbuka** — proyek JSON, template & plugin efek terdokumentasi, ekspor FCPXML/OTIO/EDL/SRT.
10. **Batch & multi-rasio** — satu proyek → 16:9, 9:16, 1:1 sekaligus, masing-masing dengan reframe otomatis.

## 3. Prinsip rekayasa

- **Clean-room.** Concat (AGPL) dan CapCut (proprietary) hanya referensi fungsional/arsitektural. Tidak ada kode, shader, aset, atau model yang disalin.
- **Ringan dulu.** Setiap dependensi wajib lulus anggaran ukuran & RAM (§6.4).
- **Satu sumber kebenaran untuk edit.** Semua perubahan proyek = *Command* (UI, Copilot, MCP, skrip memakai jalur yang sama).
- **Waktu rasional.** Timestamp = pecahan (num/den), bukan float → frame-accurate.
- **Model on-demand.** Installer tanpa bobot AI; diunduh saat fitur dipakai, diverifikasi SHA-256.
- **Offline by default, telemetri opt-in.**

## 4. Stack teknologi (rekomendasi)

| Lapisan | Pilihan | Alasan | Alternatif yang ditolak |
|---|---|---|---|
| Shell desktop | **Tauri 2** (Rust) | Biner kecil, memakai WebView2 bawaan Windows, bundler NSIS/MSI bawaan, updater bertanda tangan | Electron (±150 MB+ & RAM tinggi); Qt (lisensi/berat); Slint (iterasi UI lambat — lihat pengalaman Concat) |
| UI | **Svelte 5 + TypeScript + Vite** | Runtime sangat kecil, reaktivitas granular cocok untuk inspector besar | React (lebih berat, re-render timeline); Vue (setara, ekosistem editor lebih kecil) |
| Timeline & kanvas UI | Canvas 2D / WebGL kustom, virtualisasi | DOM tidak sanggup ribuan klip + waveform | Library timeline pihak ketiga (kurang kontrol) |
| Engine inti | **Rust** (crate workspace milik sendiri) | Performa native, aman memori, satu bahasa dengan Tauri | C++ (risiko memori), Go (interop media lemah) |
| Media I/O | **FFmpeg 7** (link dinamis via `ffmpeg-next`/`rsmpeg`), build **LGPL shared** | Semua format; HW decode D3D11VA/NVDEC/QSV; HW encode NVENC/QSV/AMF | Spawn `ffmpeg.exe` (lambat untuk preview, sulit kontrol) |
| Compositor | **wgpu** (DX12/Vulkan) + shader **WGSL** | Cepat, lintas vendor GPU; WGSL dipakai juga oleh WebGPU di preview | OpenGL (usang di Windows) |
| Preview | **WebCodecs + WebGPU** di WebView2 (Opsi A) — lihat §5.3 | Decode HW di browser engine, paket terenkode kecil dikirim dari Rust | Kirim frame mentah via IPC (4K mustahil) |
| Audio | Mixer Rust + `cpal` (WASAPI) sebagai **master clock** | Mixer yang sama untuk playback & ekspor | Web Audio (beda hasil dengan ekspor) |
| AI runtime | **ONNX Runtime** (`ort`) + **DirectML**; whisper.cpp (`whisper-rs`, Vulkan/CUDA); sherpa-onnx (TTS) | Satu runtime untuk mayoritas model; DirectML jalan di NVIDIA/AMD/Intel | PyTorch/Python (berat, prasyarat) |
| LLM lokal (opsional) | Ollama / llama.cpp server (dideteksi), atau BYOK (OpenAI-compatible, Anthropic) | Copilot tanpa membundel model besar | Membundel LLM di installer (GB) |
| Penyimpanan | Proyek: JSON (+ jurnal command); cache/indeks: SQLite | Transparan, mudah di-diff, recoverable | Format biner tertutup |
| Installer | **Tauri NSIS** (`setup.exe`), per-user, WebView2 *embedBootstrapper* | Wizard standar, tanpa admin, tanpa prasyarat | MSI saja (UX kurang ramah) |
| Code signing | Azure Trusted Signing (atau sertifikat OV) | Hindari SmartScreen — kelemahan nyata Concat | Unsigned |
| CI/CD | GitHub Actions (windows-latest) | Build, test, sign, rilis, mirror model | — |

### 4.1 Model AI yang direncanakan (semua on-demand, lisensi permisif diprioritaskan)

| Fitur | Model kandidat | Ukuran ± | Lisensi (verifikasi di Fase 0) |
|---|---|---|---|
| Speech-to-text | Whisper base / small / large-v3-turbo (GGML) | 150 MB – 1,6 GB | MIT |
| VAD / jeda | Silero VAD | 2 MB | MIT |
| TTS | Kokoro v1.0 (multi), **suara VITS Indonesia** | 80–350 MB / 60 MB | Apache-2.0 / per-suara — ⚠️ phonemizer espeak-ng = GPL-3.0, lihat [05](05-license-compliance.md) §4 |
| Voice clone | Chatterbox Turbo / F5-TTS | 300 MB – 1,2 GB | MIT / CC-BY-NC (F5 bobot → hindari) |
| Noise reduction | DeepFilterNet3 | 10 MB | MIT/Apache |
| Pisah vokal | HTDemucs / MDX-Net (ONNX) | 80–300 MB | MIT |
| Cutout orang/objek | BiRefNet-lite, IS-Net (MODNet hanya bila lisensi bobot terverifikasi) | 25–180 MB | MIT / Apache-2.0 |
| Smart brush | SlimSAM / EfficientSAM | 40 MB | Apache-2.0 |
| Upscale | Real-ESRGAN x4v3 | 5 MB | BSD-3 |
| Interpolasi frame | RIFE v4.x (ONNX) | 20 MB | MIT |
| Wajah/saliency (reframe) | YuNet + U²-Net-p (SCRFD ditolak: bobot non-komersial) | 1–5 MB | MIT / Apache-2.0 |
| Terjemahan | NLLB-200-distilled-600M (CTranslate2 int8) / Opus-MT | 300–600 MB | CC-BY-NC (NLLB!) → **Opus-MT (CC-BY) prioritas** |
| Smart search | SigLIP/CLIP ViT-B (ONNX) | 150–350 MB | Apache/MIT |

## 5. Arsitektur

### 5.1 Gambaran besar

```
┌──────────────────────────── Kinora.exe (Tauri 2) ─────────────────────────────┐
│  WebView2 (Svelte UI)                                                           │
│  ├─ Panels: Media · Timeline (canvas) · Inspector · Transcript · Copilot        │
│  ├─ Preview: WebCodecs VideoDecoder → WebGPU compositor (WGSL bersama)          │
│  └─ State store (read-model) ◄── event "project.changed" / "clock.tick"         │
│                     ▲  invoke(command) / Channel(binary packets)                │
├─────────────────────┼───────────────────────────────────────────────────────────┤
│  Rust core (proses yang sama)                                                    │
│  ├─ kinora-project  : model dokumen, Command, undo/redo, jurnal, serde           │
│  ├─ kinora-media    : FFmpeg demux/decode/encode, probe, thumbnail, proxy        │
│  ├─ kinora-render   : wgpu compositor (ekspor & fallback preview), WGSL          │
│  ├─ kinora-audio    : mixer, filter, cpal playback (master clock), LUFS          │
│  ├─ kinora-ai       : ort/DirectML, whisper, sherpa-onnx, model manager          │
│  ├─ kinora-jobs     : antrean job (ekspor, proxy, transkrip, enhance) + progres  │
│  ├─ kinora-api      : JSON-RPC lokal + server MCP (token, loopback)              │
│  └─ kinora-copilot  : prompt → rencana → daftar Command (dengan pratinjau diff)  │
└──────────────────────────────────────────────────────────────────────────────────┘
        │ unduh on-demand (SHA-256)              │ opsional
        ▼                                         ▼
  Mirror model (GitHub Releases)          Ollama lokal / BYOK API
```

### 5.2 Model data (sketsa konseptual)
- **Project** → `timelines[]`, `media[]` (path, hash, proxy, metadata), `fonts[]`, `brand`, `settings`.
- **Timeline** → `tracks[]` (video/audio/text/overlay/caption), `markers[]`, `canvas` (rasio, fps rasional, warna).
- **Clip** → `media_ref | text | shape | adjustment`, `in/out/start` (rasional), `speed` (konstan/kurva), `transform`,
  `keyframes{property → [t, value, easing]}`, `effects[]`, `transitions{in,out}`, `masks[]`, `audio{gain, fades, filters}`.
- **Transcript** → kata-kata dengan `start/end/confidence/speaker`, terikat ke `media_ref` (bukan ke klip) ⇒ tetap valid setelah dipotong.
- **Command** → `{op, params}` serializable; setiap command punya `apply` & `invert`. Jurnal `.kinora/journal.jsonl` untuk pemulihan crash.

### 5.3 Pipeline preview (risiko teknis terbesar → Spike S1)
- **Opsi A (utama):** Rust melakukan *demux* (bukan decode) → kirim paket terenkode (H.264/HEVC/VP9/AV1) via `tauri::ipc::Channel` →
  WebCodecs `VideoDecoder` (hardware) → `importExternalTexture` → compositor WebGPU dengan shader WGSL yang sama dengan ekspor.
  Codec yang tidak didukung WebCodecs (ProRes, DNxHD, dsb.) → **proxy otomatis** H.264.
- **Opsi B (cadangan):** compositor wgpu Rust menggambar langsung ke HWND (child window) di bawah area transparan WebView2.
- **Fallback C:** WebGL2 bila WebGPU di-blocklist driver.
- Audio dari Rust (`cpal`) = master clock; UI menerima `clock.tick` dan memilih frame yang sesuai.

### 5.4 Pipeline ekspor
Flatten timeline → per frame: decode (HW) → wgpu compositor (WGSL) → readback/zero-copy → encode (NVENC/QSV/AMF/OpenH264/SVT-AV1)
+ mixer audio Rust → mux (FFmpeg). Dijalankan sebagai job di `kinora-jobs`, dapat dibatalkan, dengan progres & ETA.

### 5.5 Struktur repositori (rencana)
```
kinora/
├─ apps/desktop/            # Tauri 2: src-tauri (Rust glue) + ui (Svelte)
├─ crates/                  # kinora-core, -project, -media, -render, -audio, -ai, -jobs, -api, -copilot
├─ effects/                 # paket efek: effect.toml + effect.wgsl + preview + fixture
├─ assets/                  # font open-license, stiker CC0, musik CC0, ikon
├─ models/manifest.toml     # daftar model + SHA-256 + lisensi
├─ installer/               # konfigurasi NSIS, bahasa wizard, ikon, EULA
├─ docs/                    # dokumen ini, ADR (Architecture Decision Records), panduan
├─ tests/golden/            # proyek uji + frame referensi
└─ .github/workflows/       # ci.yml, release.yml, models.yml
```

## 6. Kualitas, distribusi & non-fungsional

### 6.1 Installer & update
- `Kinora-x.y.z-setup.exe` (NSIS, wizard: bahasa → lisensi → lokasi → shortcut → selesai), **tanpa hak admin** (per-user), opsi "untuk semua pengguna".
- WebView2: *embedBootstrapper* (Win10 lama tanpa WebView2 tetap bisa). FFmpeg LGPL + ONNX Runtime DirectML dibundel.
- Asosiasi file `.kinora`, uninstaller bersih + opsi hapus model & cache.
- Auto-update via updater Tauri (paket ditandatangani), kanal *Stable* & *Beta*.
- Ditandatangani kode → tidak diblokir SmartScreen.

### 6.2 Persyaratan sistem (target)
- **Minimum:** Windows 10 22H2 / 11 x64, 8 GB RAM, GPU DirectX 12 (termasuk iGPU Intel UHD 620+), 2 GB disk.
- **Disarankan untuk AI:** 16 GB RAM, GPU 4 GB VRAM (NVIDIA/AMD/Intel Arc) atau NPU.

### 6.3 Strategi pengujian
- Unit & property test (Rust) untuk model waktu, command/undo, flatten.
- **Golden-frame test:** proyek uji dirender → dibandingkan dengan referensi (PSNR/SSIM ambang).
- Uji kesetaraan preview vs ekspor (shader WGSL yang sama).
- E2E UI via `tauri-driver` (WebDriver).
- Benchmark performa per rilis (start, scrub, ekspor) + smoke test installer di **Windows Sandbox** bersih.

### 6.4 Anggaran performa (gate rilis)
| Metrik | Target |
|---|---|
| Ukuran installer (tanpa model) | ≤ 120 MB |
| Cold start → layar awal | ≤ 2 dtk (SSD) |
| RAM idle dengan proyek kosong | ≤ 300 MB |
| Preview 1080p30, 3 layer + 2 efek, iGPU | ≥ 30 fps stabil |
| Scrub 4K (via proxy) | ≤ 100 ms per lompatan |
| Ekspor 1080p H.264 (HW encoder) | ≥ 2× realtime |
| Auto caption 10 menit, Whisper base, GPU | ≤ 60 dtk |

### 6.5 Keamanan & privasi
- API lokal hanya di loopback + token; MCP dimatikan secara default.
- Tidak ada telemetri default; laporan crash opt-in.
- Semua unduhan model diverifikasi SHA-256; mirror milik proyek + fallback upstream.

### 6.6 Desain UI/UX
- **Prioritas:** ramah pengguna untuk orang awam *dan* profesional, dengan gaya clean, estetis, dan menarik — bukan "AI slop" (gradien ungu generik, kartu seragam, ikon dekoratif tanpa fungsi).
- **Alat wajib:** skill [Impeccable](https://impeccable.style) dipakai sejak baris UI pertama (Fase 1), bukan di akhir:
  1. `npx impeccable install` di root repo.
  2. `/impeccable init` untuk memberi konteks produk & arah desain (persona §1, prinsip di bawah).
  3. `/impeccable hooks on` agar pemeriksaan desain otomatis berjalan saat agen bekerja. Hook perlu disetujui pemilik repo di pengaturan hooks Claude Code.
- **Prinsip produk:** progressive disclosure (mode sederhana di depan, kontrol pro satu klik di belakang); setiap aksi umum ≤ 2 klik; status job AI selalu terlihat dan jujur (ETA, bisa dibatalkan); dark & light theme; kontras WCAG AA; keyboard-first untuk pro; bahasa UI Indonesia yang natural, bukan terjemahan kaku.
- **Validasi:** uji kegunaan singkat dengan 3 persona (kreator awam, podcaster, editor pro) di setiap akhir fase.

## 7. Roadmap bertahap

> Asumsi: 2 developer full-stack (Rust + TS) dibantu AI. Estimasi kasar; dikalibrasi ulang setelah Fase 0.

### Fase 0 — Fondasi & de-risking (±2–3 minggu)
- Keputusan §9, nama & identitas, lisensi, repo + CI kosong, ADR pertama.
- **Spike S1 Preview:** demux Rust → WebCodecs → WebGPU, 1080p & 4K, H.264/HEVC. *Lulus jika* ≥ 30 fps 1080p di iGPU & sinkron A/V ≤ 1 frame.
- **Spike S2 Media:** FFmpeg LGPL ter-bundle, HW decode/encode di NVIDIA/Intel/AMD. *Lulus jika* ekspor 1080p ≥ 2× realtime.
- **Spike S3 AI:** ORT + DirectML (BiRefNet) & whisper.cpp Vulkan. *Lulus jika* jalan tanpa instalasi tambahan apa pun.
- **Spike S4 Installer:** setup.exe ditandatangani, dipasang di Windows Sandbox bersih. *Lulus jika* ≤ 120 MB dan aplikasi terbuka tanpa prasyarat.
- **Gate:** jika S1 gagal → pakai Opsi B (§5.3) dan revisi estimasi.

### Fase 1 — MVP Editor Inti (±8–10 minggu) → rilis *Alpha*
Langkah pertama sebelum menulis UI: pasang & inisialisasi Impeccable (§6.6). Lalu impor & media bin, proxy, timeline multi-track (split/trim/ripple/magnetic/snap), undo/redo + jurnal + autosave,
preview, transform/crop/rotate/opacity, teks dasar + font, volume/fade/waveform, ekspor H.264/HEVC hingga 4K + preset platform,
i18n ID/EN, profil performa otomatis, installer signed.
**Exit:** pengguna awam dapat membuat & mengekspor video 9:16 dengan teks & musik dalam < 10 menit tanpa crash.

### Fase 2 — Toolkit Kreatif (±6–8 minggu) → *Beta 1*
Keyframe + editor kurva, speed & curve speed, freeze/reverse, compound clip, marker, filter/efek/transisi (target ≥ 120 paket WGSL buatan sendiri),
adjust & color wheels & HSL & curves, LUT `.cube`, scopes, mask bentuk, chroma/luma key, blend mode, motion blur,
animasi klip & teks, curve text, stiker/emoji/GIF/Lottie, voiceover, efek suara, EQ/compressor, LUFS, shortcut kustom, safe-zone overlay.

### Fase 3 — AI Lokal (±8–10 minggu) → *Beta 2*
Model Manager (rekomendasi sesuai hardware), auto captions + gaya viral + highlight kata, SRT/VTT/ASS, bilingual & terjemah offline,
**edit berbasis transkrip**, **hapus filler & jeda**, noise reduction, isolate voice, auto ducking, beat detection,
TTS (termasuk Indonesia) & voice clone, remove background + smart brush, **auto reframe**, enhance/upscale, frame interpolation, stabilize,
color match, auto adjust, mask/object tracking.

### Fase 4 — Pembeda & Produktivitas (±6–8 minggu) → *Release Candidate*
**AI Copilot** (Ollama/BYOK → Command), **long video → shorts**, AI writer (judul/caption/hashtag dari transkrip),
**server MCP + JSON-RPC**, antrean & ekspor batch multi-rasio, template proyek terbuka, brand kit, plugin SDK efek,
multi-camera sync, rekam layar + webcam + teleprompter, smart search lokal, riwayat versi, ekspor FCPXML/OTIO/EDL, ProRes/DNxHR,
translate + dubbing lokal.

### Fase 5 — Hardening & Rilis 1.0 (±4 minggu)
Optimasi performa sesuai §6.4, audit aksesibilitas, dokumentasi & tutorial (ID/EN), website unduhan, auto-update kanal Stable,
beta publik → perbaikan bug → **v1.0**.

### Pasca 1.0 (P5+)
HDR end-to-end, relight, retouch wajah, inpainting video, AI music lokal, konektor BYOK generatif (text/image-to-video),
upload langsung ke platform, C2PA, versi macOS/Linux (Tauri mendukungnya).

## 8. Register risiko

| # | Risiko | Dampak | Kemungkinan | Mitigasi |
|---|---|---|---|---|
| R1 | Preview WebCodecs/WebGPU tidak stabil di sebagian driver | Tinggi | Sedang | Spike S1; fallback WebGL2; Opsi B wgpu native |
| R2 | Ukuran installer membengkak (FFmpeg + ORT) | Sedang | Sedang | Build FFmpeg minimal (hanya codec perlu), ORT DirectML saja, kompresi LZMA |
| R3 | Lisensi model/codec tidak kompatibel | Tinggi | Sedang | Tabel lisensi wajib per model; hindari GPL/NC kecuali D1 = GPL |
| R4 | Paten codec (HEVC/H.264 software encode) | Sedang | Rendah | Utamakan encoder hardware (lisensi ditanggung vendor) + OpenH264 |
| R5 | Scope creep menuju paritas 100% CapCut | Tinggi | Tinggi | Matriks fitur = kontrak; fitur cloud generatif ✖ |
| R6 | Performa laptop low-end | Sedang | Tinggi | Profil performa, proxy otomatis, resolusi preview adaptif |
| R7 | AI lokal lambat tanpa GPU | Sedang | Tinggi | Model tier kecil default, job latar belakang, estimasi waktu jujur |
| R8 | Klaim pelanggaran merek/hak cipta | Tinggi | Rendah | Clean-room, tanpa aset/merek CapCut, nama & ikon orisinal |
| R9 | Sertifikat code signing tertunda | Sedang | Sedang | Urus di Fase 0 (Azure Trusted Signing butuh verifikasi identitas) |

## 9. Keputusan

### 9.1 Status keputusan (diperbarui 19 Okt 2026)

| # | Status | Hasil | Rujukan |
|---|---|---|---|
| D1 | ✅ **Diputuskan** | Opsi (a): bangun baru, clean-room, lisensi permisif **MIT OR Apache-2.0** (dual) | [ADR-0002](adr/0002-license-and-clean-room.md) |
| D2 | ✅ **Diputuskan** | FFmpeg **LGPL shared** (tanpa x264/x265/GPL filter) | [ADR-0005](adr/0005-codec-and-ffmpeg-policy.md) |
| D3 | ✅ **Diputuskan** | **Svelte 5** + TypeScript + Vite | [ADR-0001](adr/0001-technology-stack.md) |
| D4 | ⏳ Default sementara | Nama kerja "Kinora"; cek merek/domain di Fase 0 (P0-01) | [Fase 0](04-phase0-plan.md) |
| D5 | ⏳ Default sementara | Gratis penuh; fitur lokal tidak pernah dikunci | — |
| D6 | ⏳ Default sementara | Asumsi 2 developer; dikalibrasi ulang setelah spike | — |

### 9.2 Opsi awal (arsip)

| # | Keputusan | Opsi | Rekomendasi |
|---|---|---|---|
| **D1** | **Strategi & lisensi** | (a) Bangun baru, clean-room, lisensi permisif/proprietary-gratis · (b) Bangun baru, open source GPL-3.0 · (c) **Fork Concat** (wajib AGPL) | **(a) atau (b)**. Fork (c) memang tercepat, tetapi mewarisi UI Slint & monolit 11k baris, wajib AGPL, dan pembeda kita jadi tipis. |
| D2 | Build FFmpeg | LGPL (tanpa x264/x265) · GPL (dengan x264/x265) | Ikuti D1: LGPL untuk (a); GPL boleh untuk (b) |
| D3 | Framework UI | Svelte 5 · React 19 · SolidJS | Svelte 5 |
| D4 | Nama & identitas | "Kinora" (placeholder) atau usulan Anda | Cek merek & domain di Fase 0 |
| D5 | Model bisnis jangka panjang | Gratis penuh (donasi/sponsor) · gratis + add-on opsional (mis. aset premium komunitas) | Gratis penuh untuk semua fitur di matriks; jangan pernah kunci fitur lokal |
| D6 | Kapasitas tim & target waktu | — | Konfirmasi agar estimasi §7 dikalibrasi |

## 10. Langkah berikutnya
Rencana eksekusi Fase 0 (backlog, spike S1–S4, gate go/no-go) ada di **[04-phase0-plan.md](04-phase0-plan.md)**.
Kepatuhan lisensi ada di **[05-license-compliance.md](05-license-compliance.md)**. ADR ada di folder **[adr/](adr/)**.
