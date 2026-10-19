# 02 — Matriks Fitur: CapCut vs Concat vs Aplikasi Kita

Kode nama kerja aplikasi: **"Kinora"** *(placeholder — nama final ditentukan di Fase 0, lihat 03 §9)*.

**Legenda**
- CapCut: ✅ gratis · 💎 Pro (string paywall teramati) · 🪙 kredit/menit · ☁️ butuh cloud/akun · ❔ status gating belum teramati
- Concat: ✅ ada · ⚠️ parsial · ❌ tidak ada
- Fase kita: **P1** MVP inti · **P2** kreatif · **P3** AI lokal · **P4** pembeda · **P5+** setelah rilis · **✖** di luar cakupan
- Semua fitur Kinora **gratis, offline, tanpa akun, tanpa watermark** kecuali disebut "BYOK" (pengguna memakai API key miliknya sendiri, opsional).

## A. Proyek & media

| Fitur | CapCut | Concat | Kinora | Fase | Catatan implementasi |
|---|:-:|:-:|:-:|:-:|---|
| Impor video/foto/audio (drag & drop) | ✅ | ✅ | ✅ | P1 | FFmpeg demux; thumbnail + filmstrip + waveform di background |
| Format RAW/HEIF/JPEG-XL/ProRes | ✅ | ⚠️ | ✅ | P2 | FFmpeg + libraw; ProRes → proxy otomatis |
| Media bin, folder, pencarian | ✅ | ✅ | ✅ | P1 | |
| Smart search (cari berdasarkan isi, "pantai", "orang tertawa") | ☁️ | ❌ | ✅ | P4 | Embedding CLIP/SigLIP lokal + indeks transkrip |
| Proxy otomatis untuk 4K/HEVC/ProRes | ✅ | ✅ | ✅ | P1 | |
| Autosave + pemulihan crash | ✅ | ⚠️ | ✅ | P1 | Jurnal command (append-only) → pemulihan sampai edit terakhir |
| Riwayat versi proyek (snapshot) | 💎☁️ | ❌ | ✅ | P4 | Snapshot lokal bertanggal, bisa dibandingkan & dipulihkan |
| Relink media hilang | ✅ | ✅ | ✅ | P2 | |
| Multi-timeline dalam satu proyek | ❌ | ✅ | ✅ | P2 | Mis. versi 16:9 + 9:16 dari satu proyek |
| Cloud space / teams / share review | 💎☁️ | ❌ | ✖ | — | Diganti "Paket Proyek" (.kinora zip) + link review lokal di P5+ |
| Interop NLE (FCPXML / EDL / OTIO) | ❌ | ❌ | ✅ | P4 | Pembeda: lanjut finishing di DaVinci/Premiere |

## B. Timeline & editing inti

| Fitur | CapCut | Concat | Kinora | Fase | Catatan |
|---|:-:|:-:|:-:|:-:|---|
| Multi-track video/audio/teks/overlay | ✅ | ✅ | ✅ | P1 | |
| Split, trim, ripple delete, roll | ✅ | ✅ | ✅ | P1 | |
| Magnetic timeline / snapping | ✅ | ✅ | ✅ | P1 | |
| Undo/redo tak terbatas | ✅ | ✅ | ✅ | P1 | Command pattern |
| Copy/paste atribut & efek | ✅ | ⚠️ | ✅ | P2 | |
| Freeze frame, reverse, mirror, rotate, crop | ✅ | ✅ | ✅ | P1–P2 | |
| Speed konstan + curve speed (ramp) | ✅ | ✅ | ✅ | P2 | |
| Keyframe + editor kurva (easing) | ✅ | ✅ | ✅ | P2 | |
| Compound/nested clip | ✅ | ⚠️ (consolidate) | ✅ | P2 | |
| Multi-camera clip (sinkron audio) | ✅ | ❌ | ✅ | P4 | Sinkron otomatis via cross-correlation audio |
| Marker & chapter | ✅ | ❌ | ✅ | P2 | Ekspor chapter YouTube |
| Shortcut kustom (preset Premiere/CapCut) | ✅ | ⚠️ | ✅ | P2 | |
| Grup/link klip | ✅ | ❌ | ✅ | P2 | |

## C. Visual: transformasi, efek, warna

| Fitur | CapCut | Concat | Kinora | Fase | Catatan |
|---|:-:|:-:|:-:|:-:|---|
| Transform, opacity, blend mode | ✅ | ✅ | ✅ | P1–P2 | |
| Filter/looks | ✅/💎 | ✅ (±40) | ✅ | P2 | Semua dibuat sendiri (WGSL), tanpa aset CapCut |
| Efek video (glitch, glow, blur…) | ✅/💎 | ✅ | ✅ | P2 | |
| Transisi | ✅/💎 | ✅ (±18) | ✅ | P2 | |
| Animasi masuk/keluar/loop klip | ✅/💎 | ❌ | ✅ | P2 | Preset keyframe |
| Adjust: exposure, contrast, WB, HSL, curves, color wheels | ✅ | ✅ | ✅ | P2 | |
| LUT `.cube` impor | ✅ | ❔ | ✅ | P2 | |
| Color match antar klip | ✅ | ❌ | ✅ | P3 | Transfer statistik warna (Reinhard/LAB) — tanpa AI berat |
| Auto adjust / auto enhance warna | ✅ | ❌ | ✅ | P3 | |
| Scopes (waveform, vectorscope, histogram) | ❌ | ⚠️ (meters) | ✅ | P2 | Pembeda untuk kreator serius |
| Mask bentuk (linear, mirror, circle, rect, heart, star) | ✅ | ❌ | ✅ | P2 | |
| Mask tracking / object tracking | ✅ | ❌ | ✅ | P3 | Tracker OpenCV (CSRT/KCF) atau model ringan |
| Chroma key / luma key | ✅ | ✅ | ✅ | P2 | |
| Stabilize | ✅ | ❌ | ✅ | P3 | Implementasi sendiri (feature tracking + smoothing jalur kamera); `libvidstab` GPL → tidak boleh (ADR-0005) |
| Motion blur | ✅ | ✅ | ✅ | P2 | |
| Optical flow slow-mo / frame interpolation | ✅ | ❌ | ✅ | P3 | RIFE (ONNX) |
| HDR (HLG/PQ) & tone-mapping | ✅ | ⚠️ | ✅ | P5+ | |
| Relight | ✅/🪙 | ❌ | ⚠️ | P5+ | Riset; mungkin berbasis depth (Depth-Anything) |

## D. Teks, grafis, caption

| Fitur | CapCut | Concat | Kinora | Fase | Catatan |
|---|:-:|:-:|:-:|:-:|---|
| Teks: font, stroke, shadow, background, spacing | ✅ | ✅ | ✅ | P1 | |
| Stroke inside/center/outside, multi-stroke | ✅ | ⚠️ | ✅ | P2 | |
| Curve text | ✅ | ❌ | ✅ | P2 | |
| Animasi teks (typewriter, pop, bounce, karaoke) | ✅/💎 | ❌ | ✅ | P2 | |
| Template teks / title presets | ✅/💎 | ✅ | ✅ | P2 | |
| Stiker, emoji, GIF stiker, shapes | ✅ | ⚠️ (shapes) | ✅ | P2 | Pack open-license (OpenMoji/Twemoji CC-BY) + impor GIF/APNG/Lottie |
| Font Google bawaan + impor font | ✅ | ✅ | ✅ | P1 | |
| **Auto captions** (STT) | 💎 | ✅ | ✅ | P3 | whisper.cpp + GPU (Vulkan/CUDA) |
| Gaya caption viral (highlight kata aktif, emoji otomatis) | 💎 | ⚠️ | ✅ | P3 | Word-level timestamps |
| Auto highlight keyword | 💎 | ❌ | ✅ | P3 | Heuristik + opsional LLM lokal |
| Caption bilingual & terjemah | 💎🪙 | ❌ | ✅ | P3 | NLLB-200 distilled / Opus-MT via CTranslate2 (offline) |
| Auto lyrics (lagu) | 💎 | ❌ | ✅ | P3 | Whisper + vocal separation dulu |
| Impor/ekspor SRT/VTT/ASS | ✅ | ⚠️ | ✅ | P3 | |
| Burn-in atau soft-subtitle saat ekspor | ✅ | ⚠️ | ✅ | P3 | |

## E. Audio

| Fitur | CapCut | Concat | Kinora | Fase | Catatan |
|---|:-:|:-:|:-:|:-:|---|
| Volume, fade, keyframe volume | ✅ | ✅ | ✅ | P1 | |
| Waveform & meter | ✅ | ✅ | ✅ | P1 | |
| Voiceover dari mikrofon | ✅ | ✅ | ✅ | P2 | |
| Teleprompter saat rekam | ✅ | ❌ | ✅ | P4 | |
| Rekam layar + webcam | ✅ | ❌ | ✅ | P4 | Windows Graphics Capture API |
| Voice effects / voice changer | ✅/💎 | ✅ | ✅ | P2 | Rantai filter FFmpeg |
| Noise reduction | 💎 | ✅ | ✅ | P3 | DeepFilterNet3 (ONNX, MIT/Apache) |
| Isolate voice / pisah vokal-instrumen | 💎 | ❌ | ✅ | P3 | HTDemucs / MDX-Net (ONNX) |
| Loudness normalization (LUFS, preset platform) | ✅ | ✅ | ✅ | P2 | EBU R128; preset −14 LUFS (YouTube/TikTok) |
| Auto ducking (musik turun saat ada suara) | ✅ | ❌ | ✅ | P3 | VAD Silero |
| Beat detection / auto beat markers | ✅ | ❌ | ✅ | P3 | aubio/onset detection |
| Equalizer, compressor, reverb | ⚠️ | ✅ | ✅ | P2 | |
| Library musik & SFX | ✅/💎☁️ | ❌ | ⚠️ | P4 | Pack CC0 bawaan + konektor Pixabay/Freesound (opsional) |
| AI music / AI sound effects | 🪙☁️ | ❌ | ⚠️ | P5+ | BYOK atau model lokal (MusicGen-small) eksperimental |

## F. AI

| Fitur | CapCut | Concat | Kinora | Fase | Model/implementasi (lokal) |
|---|:-:|:-:|:-:|:-:|---|
| **Edit berbasis transkrip** (hapus kata = hapus video) | ✅/💎 | ❌ | ✅ | P3 | Pembeda utama vs Concat |
| Hapus filler words & jeda | 💎 | ❌ | ✅ | P3 | Whisper word-ts + VAD + kamus filler per bahasa (termasuk "eee", "anu", "jadi") |
| Remove background (orang) | 💎 | ✅ | ✅ | P3 | BiRefNet-lite / MODNet (lisensi permisif, bukan RVM GPL) |
| Remove background (objek) | 💎 | ✅ | ✅ | P3 | BiRefNet / IS-Net |
| Smart brush / smart eraser (SAM) | ✅/💎 | ✅ | ✅ | P3 | SlimSAM / EfficientSAM |
| Object removal (inpainting video) | 🪙 | ❌ | ⚠️ | P5+ | LaMa (gambar) dulu; video inpainting = riset |
| Auto reframe (16:9 → 9:16, ikuti subjek) | 💎 | ❌ | ✅ | P3 | Deteksi wajah/saliency + smoothing jalur kamera |
| Enhance / upscale | 💎 | ✅ | ✅ | P3 | Real-ESRGAN x4v3 (offline job) |
| Retouch wajah/tubuh | ✅/💎 | ❌ | ⚠️ | P5+ | Face mesh + smoothing shader |
| Text-to-speech | ✅/🪙 | ✅ | ✅ | P3 | Kokoro + **Piper (suara Bahasa Indonesia)** |
| Voice clone | 💎🪙 | ✅ | ✅ | P3 | Chatterbox / F5-TTS (cek lisensi bobot) |
| Translate video + dubbing | 🪙 | ❌ | ✅ | P4 | STT → NLLB → TTS clone → time-stretch per kalimat |
| Lip sync | 🪙☁️ | ❌ | ✖ | — | Butuh model besar; di luar cakupan |
| **Long video → shorts** (highlight) | 🪙 | ❌ | ✅ | P4 | Transkrip + skor (energi audio, kata kunci, LLM lokal/BYOK) |
| **AI Copilot / agent edit dengan prompt** | 🪙 (EditPilot) | ❌ | ✅ | P4 | LLM lokal (Ollama) atau BYOK → memanggil Command API yang sama |
| Script to video / text to video / image to video | 🪙☁️ | ❌ | ⚠️ | P5+ | Hanya via BYOK (konektor penyedia), bukan bawaan |
| AI avatar, digital human, fashion model | 🪙☁️ | ❌ | ✖ | — | Di luar cakupan (biaya server, risiko deepfake) |
| AI writer / copywriting (judul, caption sosmed, hashtag) | 💎 | ❌ | ✅ | P4 | LLM lokal/BYOK, input = transkrip |
| Auto B-roll | 🪙 | ❌ | ⚠️ | P5+ | Smart search lokal ke media bin pengguna |

## G. Ekspor & distribusi

| Fitur | CapCut | Concat | Kinora | Fase | Catatan |
|---|:-:|:-:|:-:|:-:|---|
| H.264 / HEVC / AV1 | ✅ | ✅ | ✅ | P1 | NVENC / QSV / AMF / software |
| Hingga 4K60, 10-bit | ✅/💎 | ✅ | ✅ | P1–P2 | |
| ProRes / DNxHR (intermediate) | ⚠️ | ❌ | ✅ | P4 | |
| GIF / WebM / audio-only (MP3/WAV) | ✅ | ⚠️ | ✅ | P2 | |
| Tanpa watermark | 💎 | ✅ | ✅ | P1 | Selalu |
| Ekspor batch / antrean | ✅ | ⚠️ (jobs) | ✅ | P4 | Satu proyek → banyak rasio/preset sekaligus |
| Preset platform (TikTok, Reels, Shorts, YouTube) | ✅ | ❌ | ✅ | P2 | |
| Safe-zone overlay UI TikTok/Reels | ✅ | ❌ | ✅ | P2 | |
| Upload langsung ke TikTok/YouTube | ✅☁️ | ❌ | ⚠️ | P5+ | OAuth opsional |
| Content credentials (C2PA) | ✅ | ❌ | ⚠️ | P5+ | Opsional, transparansi konten AI |

## H. Platform & ekosistem

| Fitur | CapCut | Concat | Kinora | Fase | Catatan |
|---|:-:|:-:|:-:|:-:|---|
| Installer satu klik, tanpa prasyarat | ✅ | ⚠️ (unsigned) | ✅ | P1 | NSIS + code signing |
| Ukuran instalasi | ±1,6 GB | ±140 MB | **target ≤ 120 MB** (tanpa model) | P1 | |
| Model AI on-demand + manajer model | ❌ (bundled) | ✅ | ✅ | P3 | Rekomendasi otomatis sesuai GPU/RAM |
| Auto-update bertanda tangan | ✅ | ✅ | ✅ | P5 | |
| Bahasa UI (ID & EN dulu) | ✅ | ✅ (25) | ✅ | P1 | i18n sejak hari pertama |
| Template proyek (slot placeholder) | ✅/💎☁️ | ✅ | ✅ | P4 | Format terbuka `.kinoratpl` |
| Brand kit (logo, warna, font, intro/outro) | 💎 | ❌ | ✅ | P4 | |
| Plugin efek (WGSL + manifest) | ❌ | ⚠️ (internal) | ✅ | P4 | SDK publik |
| API lokal + **server MCP** (AI agent eksternal) | ❌ | ✅ (JSON-RPC) | ✅ | P4 | Pembeda: Claude/agent lain bisa mengedit |
| Mode performa (laptop lemah / iGPU) | ⚠️ | ⚠️ | ✅ | P1 | Profil otomatis: resolusi preview, proxy, cache |

## Ringkasan cakupan

- **Paritas penuh dengan Concat** + **±85% fitur editing & AI lokal CapCut** pada akhir P4.
- **Sengaja tidak dibuat:** fitur generatif berbasis cloud milik CapCut (avatar, lip sync, text/image-to-video bawaan, cloud teams). Pengganti: konektor **BYOK** opsional.
