# 05 — Kebijakan Kepatuhan Lisensi

> Berlaku untuk seluruh kode, dependensi, model AI, dan aset Kinora. Dasar: [ADR-0002](adr/0002-license-and-clean-room.md), [ADR-0005](adr/0005-codec-and-ffmpeg-policy.md).
> **Disclaimer:** ini kebijakan rekayasa, bukan nasihat hukum. Sebelum rilis 1.0, minta tinjauan konsultan hukum HKI.

## 1. Kelas lisensi

| Kelas | Contoh | Kode/biner yang di-link | Model/aset yang dibundel atau diunduh |
|---|---|---|---|
| 🟢 **Diizinkan** | MIT, MIT-0, Apache-2.0 (± LLVM-exception), BSD-2/3, 0BSD, ISC, Zlib, Unicode-3.0, BSL-1.0, BlueOak-1.0.0, Unlicense, Python-2.0 (tooling JS), MPL-2.0 (file-level), OFL-1.1 (font), CC0, CC-BY-4.0 (atribusi) | ✅ | ✅ (atribusi dicatat) |
| 🟡 **Bersyarat** | LGPL-2.1/3.0 | ✅ hanya **link dinamis** (DLL dapat diganti) + notice + tautan source | ✅ dengan notice |
| 🟡 **Bersyarat** | CC-BY-SA-4.0 | ❌ | ⚠️ hanya aset, tidak dimodifikasi, notice SA |
| 🔴 **Dilarang** | GPL-2/3, AGPL-3, SSPL, CC-BY-NC(-*), "research only", lisensi custom non-komersial, tanpa lisensi | ❌ | ❌ |

## 2. Penegakan otomatis (aktif sejak P0-04)
- **Rust:** `cargo-deny` dengan [`deny.toml`](../deny.toml): allow-list kelas 🟢, larangan eksplisit crate GPL (x264, x265, espeak), sumber hanya crates.io. FFmpeg (LGPL) adalah DLL, bukan crate, jadi tidak muncul di sini. CI job `cargo-deny`.
- **JS:** [`scripts/check-js-licenses.mjs`](../scripts/check-js-licenses.mjs) (`pnpm run licenses:check`) membaca `pnpm licenses list` dan mengevaluasi ekspresi SPDX (`OR`/`AND`). CI job `web`.
- **Advisory keamanan:** `cargo deny check advisories` tiap push ke `main` dan mingguan; tidak memblokir PR yang tidak terkait.
- **Model:** `models/manifest.toml` wajib berisi `license`, `license_url`, `sha256`, `upstream_commit`; CI menolak entri tanpa lisensi 🟢 (atau 🟡 yang disetujui).
- **Aset:** `assets/**/LICENSE.json` per pack; CI menolak file aset tanpa entri.
- `THIRD_PARTY_NOTICES.md` dibangkitkan otomatis (cargo-about + skrip JS + manifest model) dan ikut di installer (halaman "Tentang → Lisensi").

## 3. Audit awal komponen (status 19 Okt 2026 — diverifikasi ulang di Fase 0)

### 3.1 Runtime & library
| Komponen | Lisensi | Kelas | Catatan |
|---|---|---|---|
| Tauri 2, wry, tao | MIT/Apache-2.0 | 🟢 | |
| Svelte 5, Vite | MIT | 🟢 | |
| wgpu, naga | MIT/Apache-2.0 | 🟢 | |
| cpal | Apache-2.0 | 🟢 | |
| FFmpeg 7 (build LGPL) | LGPL-2.1+ | 🟡 | DLL dinamis; skrip build dipublikasikan |
| `ffmpeg-next` / `rsmpeg` | WTFPL / MIT | 🟢 | binding |
| ONNX Runtime + DirectML | MIT | 🟢 | DirectML redistributable — cek ketentuan redistribusi |
| whisper.cpp / whisper-rs | MIT / Unlicense | 🟢 | |
| sherpa-onnx | Apache-2.0 | 🟢 / ⚠️ | **Bergantung pada espeak-ng (GPL-3.0)** untuk sebagian model TTS → lihat §4 |
| OpenH264 | BSD-2 (+ lisensi paten Cisco bila biner diunduh dari Cisco) | 🟢 | Unduh saat runtime |
| SVT-AV1, dav1d, libvpx, libopus | BSD | 🟢 | |
| libmp3lame | LGPL | 🟡 | via FFmpeg dinamis |
| x264, x265, libvidstab, espeak-ng | GPL | 🔴 | **Tidak dipakai** |
| Concat (seluruh repo) | AGPL-3.0 | 🔴 | Referensi clean-room saja |

### 3.2 Model AI
| Model | Lisensi bobot | Kelas | Keputusan |
|---|---|---|---|
| Whisper (GGML) | MIT | 🟢 | Pakai |
| Silero VAD | MIT | 🟢 | Pakai |
| Kokoro-82M v1.0 | Apache-2.0 | 🟢 | Pakai (G2P: §4) |
| Chatterbox Turbo | MIT | 🟢 | Pakai untuk voice clone (verifikasi watermark Perth bawaan) |
| F5-TTS | CC-BY-NC-4.0 | 🔴 | **Tolak** |
| DeepFilterNet3 | MIT/Apache-2.0 | 🟢 | Pakai |
| HTDemucs | MIT | 🟢 | Pakai |
| BiRefNet | MIT | 🟢 | Pakai (cutout utama) |
| IS-Net (DIS) | Apache-2.0 | 🟢 | Pakai |
| MODNet | Kode Apache-2.0; **lisensi bobot belum terverifikasi** | ⚠️ | Verifikasi; jika tidak jelas → tolak |
| RobustVideoMatting | GPL-3.0 | 🔴 | **Tolak** |
| SlimSAM / EfficientSAM | Apache-2.0 | 🟢 | Pakai |
| Real-ESRGAN x4v3 | BSD-3 | 🟢 | Pakai |
| RIFE v4 | MIT | 🟢 | Pakai |
| YuNet (wajah) | MIT/Apache (OpenCV Zoo) | 🟢 | Pakai |
| SCRFD / InsightFace | Bobot non-komersial | 🔴 | **Tolak** |
| U²-Net | Apache-2.0 | 🟢 | Pakai (saliency) |
| SigLIP / OpenAI CLIP | Apache-2.0 / MIT | 🟢 | Pakai (smart search) |
| NLLB-200 | CC-BY-NC-4.0 | 🔴 | **Tolak** |
| Opus-MT (Helsinki-NLP) | CC-BY-4.0 | 🟢 | Pakai (atribusi) |

### 3.3 Aset
| Aset | Lisensi | Kelas |
|---|---|---|
| Google Fonts (pilihan) | OFL-1.1 / Apache-2.0 | 🟢 |
| Microsoft Fluent Emoji | MIT | 🟢 (pilihan utama emoji/stiker) |
| Twemoji (grafik) | CC-BY-4.0 | 🟢 |
| OpenMoji | CC-BY-SA-4.0 | 🟡 (opsional) |
| Musik/SFX bawaan | **hanya CC0** | 🟢 |
| LUT & preset efek | Dibuat sendiri | 🟢 |

## 4. Masalah terbuka: phonemizer TTS (espeak-ng = GPL-3.0)
Banyak model TTS (Kokoro untuk bahasa non-Inggris, VITS/Piper) memakai **espeak-ng** untuk mengubah teks → fonem.
Menautkannya akan membuat distribusi kita terikat GPL. Rencana:
1. **Inggris:** G2P berbasis leksikon (kamus + aturan fallback) — tanpa espeak.
2. **Bahasa Indonesia:** ejaan bahasa Indonesia hampir fonemis → **G2P berbasis aturan buatan sendiri** (ng, ny, sy, kh, e-pepet vs e-taling via kamus kecil, angka/singkatan → teks). Ini sekaligus pembeda kualitas TTS Indonesia.
3. Model suara Indonesia: latih/fine-tune ringan Kokoro/VITS pada dataset berlisensi permisif (dicari di Fase 0; contoh kandidat perlu diverifikasi lisensinya) — bila tidak ada, TTS Indonesia digeser ke P4/P5.
4. Verifikasi di **Spike S3**: apakah sherpa-onnx dapat dibangun tanpa espeak-ng dan menerima input fonem/token langsung.

## 5. Checklist per pull request
- [ ] Dependensi baru lolos `cargo-deny` / pemeriksa lisensi JS.
- [ ] Model/aset baru punya entri lisensi + SHA-256.
- [ ] Tidak ada kode/aset yang disalin dari Concat atau CapCut (pernyataan di template PR).
- [ ] `THIRD_PARTY_NOTICES.md` dibangkitkan ulang bila ada perubahan.
