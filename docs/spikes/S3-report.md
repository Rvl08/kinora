# Laporan Spike S3 — Runtime AI Lokal

> **Status:** Selesai (21 Okt 2026). Diuji di Windows Sandbox (mesin bersih) dengan RTX 5070 Ti lewat GPU-PV. iGPU Intel/AMD dan perangkat 8 GB RAM **belum** diuji (perangkat tidak ada).
> **ADR terkait:** [ADR-0006](../adr/0006-local-ai-runtime.md) (baru) · **Kode:** [`spikes/s3-ai/`](../../spikes/s3-ai/README.md) · **Lanjutan:** S3b (model cutout), S4 (VC++ runtime di installer)
> **Rekomendasi:** ✅ **ORT + DirectML** dan **whisper.cpp + Vulkan** jalan di GPU tanpa CUDA, Python, atau instalasi tambahan. Caption 10 menit (base, GPU) selesai dalam ±13 detik.
> TTS tanpa espeak-ng terbukti. ⚠️ Dua risiko produk:
> (1) export BiRefNet yang diuji butuh **11 GB VRAM**, jadi model cutout default belum ada (S3b);
> (2) TTS bahasa Indonesia baru "beta" (Kokoro tidak punya suara Indonesia).

## 1. Ringkasan

| Kriteria (04 §4 S3) | Target | Hasil | Status |
|---|---|---|---|
| Jalan di GPU tanpa instalasi tambahan | Tanpa CUDA/Python | DirectML (D3D12) dan Vulkan hanya butuh driver GPU. **Tapi** VC++ runtime tidak ada di Windows bersih → installer wajib membawanya (§4, temuan 2) | ✅ / ⚠️ |
| Caption 10 menit, Whisper base, GPU | ≤ 60 dtk | **±13 dtk** (RTF 0,021). large-v3-turbo ±26 dtk (RTF 0,043) | ✅ |
| WER kasar bahasa Indonesia | 3 klip | 25 klip FLEURS (322 dtk): base **38%**, small **19%**, large-v3-turbo **4,8%** | ✅ (turbo) |
| Akurasi timestamp kata | ±80 ms | base: median 45 ms, 78% kata dalam ±80 ms (setelah perbaikan DTW, §4 temuan 4). turbo: 65 ms / 57% | 🟡 |
| TTS tanpa espeak-ng (atau G2P alternatif) | Teridentifikasi | Kokoro lewat ORT + G2P sendiri. Inggris WER bolak-balik **1,2%**, Indonesia **24–47%** | ✅ / ⚠️ id |
| Cutout ms/frame | Terukur | BiRefNet-lite fp16 DirectML **98 ms** (1024²), tetapi **11,3 GB VRAM** | ⚠️ → S3b |
| RAM/VRAM puncak | Terukur | §3.6: Whisper ≤ 1,3 GB RAM / 2,0 GB VRAM; Kokoro ≤ 0,6 / 0,7 GB; BiRefNet 11–23 GB VRAM | ✅ terukur |
| Ukuran DLL runtime (ORT + DirectML) | Terukur | `DirectML.dll` 18,5 MB (9,3 MB gzip). ORT + whisper.cpp statis dalam exe 27,6 MB (9,5 MB gzip). Backend Vulkan **+55 MB** (+16 MB gzip) | ✅ terukur |
| Unduhan model + SHA-256 + resume | Berfungsi | 17 model (1,98 GB) terverifikasi; resume dari byte 52.435.630 lolos SHA-256 | ✅ |

## 2. Lingkungan & metode

**Mengapa Windows Sandbox:**
- Mesin dev menjalankan **Smart App Control** (enforce). SAC memblokir build script Cargo dan exe yang baru dibangun dan belum ditandatangani (os error 4551).
- Pemilik mesin memutuskan SAC tetap aktif. Mematikannya permanen tanpa instal ulang Windows.
- Semua build dan pengukuran S3 berjalan di Windows Sandbox, yaitu VM sekali pakai tempat SAC hanya mengevaluasi, bukan memblokir.
- Sandbox juga berfungsi sebagai "mesin bersih" untuk kriteria S3.
- Toolchain host dipetakan read-only. Repo dipetakan ke `C:\k` agar path CMake di bawah MAX_PATH.
- Detail: [`spikes/s3-ai/README.md`](../../spikes/s3-ai/README.md).

**Mesin dan perangkat lunak:**
- **Mesin:** i7-14700F, dengan 28 CPU logis terlihat di Sandbox (RAM Sandbox 16 GB).
- **GPU:** RTX 5070 Ti 16 GB lewat **GPU-PV (vGPU)**. Angka GPU di VM bisa sedikit lebih lambat daripada bare metal.
- **Runtime:** ORT `2.0.0-rc.15` (binari Microsoft, link statis, DirectML EP); whisper.cpp lewat `whisper-rs 0.16` (Vulkan SDK 1.4.363, MSVC 14.51).

**Data uji:**
- **Indonesia:** 25 klip FLEURS `id_id` dev (CC-BY-4.0), total 322 dtk, rata-rata 13 dtk/klip.
- **Inggris:** 6 kalimat SAPI (34 dtk). Waktu awal setiap kata diketahui persis, jadi dipakai sebagai kebenaran timestamp.
- **Gambar cutout:** dua foto public-domain Wikimedia.
- **VAD:** "podcast" dari 6 kalimat SAPI dengan jeda 1,5 dtk; kebenaran diambil dari energi sinyal.

**Metode:**
- **WER:** teks dinormalisasi (huruf kecil, tanpa tanda baca, angka dieja per bahasa), lalu dihitung lewat alignment Levenshtein.
- **Memori:** working set dan commit proses, plus VRAM per proses lewat DXGI `QueryVideoMemoryInfo`. VRAM ini mencakup alokasi D3D12 dan Vulkan. Disampel setiap 50 ms, satu konfigurasi per proses.
- **Laporan mentah:** `tests/media/s3-out/runs/*` (tidak di-commit; dapat dibangkitkan ulang).

## 3. Hasil

### 3.1 Pengelola model
Ada 17 entri di [`models/manifest.toml`](../../models/manifest.toml). URL HF dipin ke commit, dan lisensi setiap entri dicatat (MIT/Apache-2.0).

| Uji | Hasil |
|---|---|
| Unduh semua | 1,98 GB, 8–11 MB/dtk (file besar), semuanya lolos SHA-256 |
| Resume | `whisper-small` diputus pada 50 MB, lalu dilanjutkan dari byte 52.435.630 (HTTP Range ke `.part`), dan file akhir lolos SHA-256 |
| File kecil non-LFS | `tokenizer.json`, `us_gold.json`, `us_silver.json`: git-blob SHA-1 dicocokkan saat unduh pertama, lalu SHA-256-nya dipin ke manifest |
| Tanpa digest | Ditolak (by design) |

### 3.2 Silero VAD v5
- Satu thread CPU, RTF **0,0021** (43 dtk audio dalam 94 ms). Memori puncak 41 MB.
- Segmen terdeteksi **12/12**.
- Galat onset −42 … +56 ms. Galat offset +50 … +160 ms; itu sudah termasuk padding 30 ms dan hangover segmenter, yang memang disengaja agar ujung kata tidak terpotong.
- I/O model: `input [-1,-1]`, `state [2,-1,128]`, `sr` int64 skalar.

### 3.3 Cutout BiRefNet-lite (input 1024², 10 run, satu konfigurasi per proses)

| Model · device | Rata-rata | Run pertama | Load | RAM puncak | VRAM puncak |
|---|---|---|---|---|---|
| fp16 · DirectML | **98 ms** | 388 ms | 4,7 dtk | 0,55 GB | **11,3 GB** |
| fp32 · DirectML | 2.712 ms | 3.370 ms | 4,6 dtk | 0,61 GB | **22,7 GB** (> 16 GB VRAM → tumpah ke memori bersama → lambat) |
| fp32 · CPU (8 thread) | 5.666 ms | 6.194 ms | 3,0 dtk | **11,9 GB** (commit 17,3) | — |
| fp16 · CPU | ❌ `bad allocation` (GatherND) | — | — | 10,2 GB (commit 18,4) | — |
| fp32 · CPU, arena dimatikan | 7.864 ms | — | — | 6,0 GB | — |
| fp16 · CPU, arena dimatikan | 8.398 ms | — | — | 6,4 GB | — |

- **Kualitas:** fraksi foreground identik (0,736) di semua konfigurasi yang berhasil. Matting rambut pada fp16/DirectML diperiksa visual dan sangat baik.
- **Penyebab memori:** export ONNX ini memakai deform-conv berbasis `GatherND` dengan input tetap 1024², sehingga tensor indeks raksasa terbentuk di decoder.
  - DirectML dengan `memory_pattern` mati (syarat DML) tidak dapat menggunakan ulang buffer.
  - Di CPU, arena ORT menahan semuanya.
- **Kesimpulan:** jalurnya (ORT + DML fp16, pekerjaan latar) terbukti, tetapi **export ini tidak layak sebagai default**. Ia menutup GPU 8 GB dan semua laptop → **S3b** (§5).

### 3.4 Whisper (whisper.cpp)
Kecepatan, sebagai RTF (waktu ÷ durasi audio; lebih kecil lebih cepat):
- Kolom Indonesia mewakili kondisi tunak (klip rata-rata 13 dtk).
- Kolom Inggris di GPU memuat **pemanasan pipeline Vulkan** pada pemakaian pertama setiap model (±10 dtk).

| Model | GPU Vulkan · id | GPU · en | CPU 12 thread (AVX2) · id | CPU · en | 10 menit audio (GPU / CPU) |
|---|---|---|---|---|---|
| base (141 MB) | **0,021** | 0,38* | 0,089 | 0,11 | **13 dtk** / 53 dtk |
| small (465 MB) | 0,030 | 0,045 | 0,221 | 0,40 | 18 dtk / 2,2 mnt |
| large-v3-turbo q5_0 (547 MB) | **0,043** | 0,36* | 0,776 | 1,72 | **26 dtk** / 7,8 mnt |

\* termasuk pemanasan Vulkan. Encoder Whisper selalu memproses jendela 30 dtk, jadi klip pendek (rata-rata 5,7 dtk untuk en) membuat RTF tampak tinggi. Audio panjang akan lebih efisien daripada angka di tabel.

**Akurasi (WER):**

| Model | Indonesia (25 klip) | Inggris (6 kalimat SAPI) |
|---|---|---|
| base | 38,3% | 3,6% |
| small | 18,8% | 3,6% |
| large-v3-turbo | **4,8%** | 1,2% (GPU) / 3,6% (CPU) |

Contoh: base mendengar "Agama utama di Moldova adalah Kristen Ortodoks" sebagai "Akam automati, mulut 2-5…", sedangkan turbo menuliskannya tepat.
**Bahasa Indonesia butuh turbo.**

**Timestamp awal kata** (73 kata Inggris yang cocok, galat terhadap waktu SAPI):

| Estimasi | base: median / p90 / ≤ 80 ms | small | turbo | Bias median |
|---|---|---|---|---|
| `t0` token (heuristik) | 95 / 260 ms / 47% | 125 ms / 37% | 90–105 ms / 44–47% | −25 ms |
| DTW token pertama kata | 250 / 500 ms / 4% | 210 ms | 220 ms | +250 ms |
| **DTW token sebelumnya** | **45 / 100 ms / 78%** | 80 / 150 ms / 56% | 65 / 160–170 ms / 57% | −40 … −80 ms |

Kebenaran berasal dari suara sintetis yang bersih. Ucapan nyata lebih sulit; timestamp bahasa Indonesia tidak diukur karena FLEURS tidak punya waktu per kata.

### 3.5 TTS Kokoro-82M (ORT langsung, tanpa sherpa-onnx, tanpa espeak-ng)
**Kecepatan** (6 kalimat Inggris, 32 dtk audio, suara `af_heart`):

| Model · device | RTF | Panggilan pertama | Load |
|---|---|---|---|
| uint8 (88 MB) · DirectML | **0,11** | 2,7 dtk (kompilasi bentuk) | 2,2 dtk |
| uint8 · CPU 8 thread | 0,24 | 1,1 dtk | 0,7 dtk |
| fp32 (310 MB) · DirectML | 0,20 | 1,8 dtk | 2,4 dtk |
| fp32 · CPU 8 thread | 0,23 | 1,3 dtk | 1,1 dtk |

**Kejelasan:** audio hasil sintesis ditranskripsi ulang oleh large-v3-turbo lalu dihitung WER-nya. Ini batas atas kasar; galat ASR ikut terhitung.

| Bahasa · suara | G2P | WER |
|---|---|---|
| en · af_heart, am_michael | kamus misaki (2 kata OOV: "Kinora", "generated") | **1,2%** |
| id · ef_dora (Spanyol, P) | aturan Kinora | 24,2% |
| id · em_alex (Spanyol, L) | aturan Kinora | 24,8% |
| id · pf_dora (Portugis, P) | aturan Kinora | 28,0% |
| id · af_heart (Inggris AS) | aturan Kinora | 41,4% |
| id · if_sara (Italia) | aturan Kinora | 46,5% |

Galat Indonesia didominasi tiga hal:
- vokal tambahan di awal kalimat setelah titik ("Etiga puluh…");
- kata Inggris di tengah teks Indonesia ("Did Not Finish"), yang dibaca dengan aturan Indonesia;
- schwa yang terdengar sebagai /o/ ("belas" → "bolas").

Suara Spanyol paling cocok dengan fonologi Indonesia.

### 3.6 Memori puncak per bagian (satu proses per bagian, GPU bila tersedia)

| Bagian | RAM (working set) | Commit | VRAM |
|---|---|---|---|
| Idle (proses kosong) | 24 MB | 12 MB | 0 |
| VAD | 41 MB | 26 MB | 0 |
| Whisper (base→turbo, GPU + CPU) | 1,33 GB | 2,20 GB | **2,03 GB** |
| Kokoro (4 konfigurasi) | 0,58 GB | 0,67 GB | 0,68 GB |
| BiRefNet | lihat §3.3 | | **11,3–22,7 GB** |

### 3.7 Simulasi CPU lemah (4 thread, tanpa GPU)
Ini batas bawah yang **optimistis**: 4 thread core desktop modern jauh lebih cepat daripada 4 core laptop murah. Uji nyata di laptop iGPU masih tertunda.

| Beban | RTF 4 thread | 10 menit audio | Dibanding 12 thread |
|---|---|---|---|
| Whisper base · id | 0,100 | ±60 dtk | ≈ sama (terbatas bandwidth memori) |
| Whisper small · id | 0,295 | ±3 mnt | 1,3× lebih lambat |
| Whisper turbo · id | 1,31 | ±13 mnt | 1,7× lebih lambat |
| Kokoro uint8 · CPU | **0,158** | — | **lebih cepat** daripada 8 thread (0,24) |
| BiRefNet fp32 · CPU | 7,3 dtk/frame, 12 GB RAM | — | tidak layak |

### 3.8 Ukuran

| Komponen | Mentah | gzip -9 |
|---|---|---|
| `DirectML.dll` | 18,5 MB | 9,3 MB |
| exe spike (ORT statis + whisper.cpp CPU + kode spike) | 27,6 MB | 9,5 MB |
| exe spike + backend Vulkan (shader SPIR-V tertanam) | 83,1 MB | 25,8 MB |
| VC++ runtime app-local (`msvcp140`, `msvcp140_1`, `vcruntime140`, `vcruntime140_1`) | 0,91 MB | 0,33 MB |

## 4. Temuan

1. **Smart App Control memblokir build lokal.** Pengembangan native di mesin dengan SAC perlu Windows Sandbox (atau penandatanganan). Konsekuensi produk: **setiap biner yang dikirim wajib ditandatangani**, termasuk DLL pihak ketiga (FFmpeg, DirectML), karena SAC juga aktif di mesin pengguna. Prioritas P0-06 naik.
2. **Windows bersih tidak punya VC++ runtime.**
   - Di Sandbox, `msvcp140.dll` tidak ada di System32.
   - Program yang dibangun dengan STL MSVC baru lalu crash (0xC0000005 di `std::mutex`) bila memuat salinan lama; `vulkan-shaders-gen` adalah contohnya.
   - Exe Kinora mengimpor `MSVCP140/VCRUNTIME140` karena whisper.cpp dan ORT.
   - Keputusan: **installer membawa runtime app-local** (S4). Spike ini berjalan karena PATH Sandbox berisi folder compiler VS, jadi kriteria "hanya file dari installer" baru terbukti di S4.
3. **`GGML_NATIVE` gagal diam-diam di Sandbox:** probe `check_c_source_runs` gagal, dan whisper.cpp terbangun **tanpa AVX** (5× lebih lambat: base RTF 0,61 → 0,12). Rilis wajib memakai flag eksplisit atau `GGML_CPU_ALL_VARIANTS` + `GGML_BACKEND_DL` (ADR-0006 §2).
4. **`t_dtw` whisper.cpp menandai akhir token.** Memakai DTW token pertama kata justru terlambat ±250 ms. DTW **token sebelumnya** memberi median 45 ms (base). Sisa bias −40 … −80 ms bisa dikoreksi konstan.
5. **Pemanasan pipeline Vulkan** ±10 dtk saat model pertama kali dipakai (shader dikompilasi driver). Solusi: pemanasan di latar setelah unduh, dan andalkan cache shader driver.
6. **Backend Vulkan menambah 55 MB** shader SPIR-V tertanam (+16 MB gzip). Itu terlalu besar untuk dibundel buta; jadikan DLL backend terpisah (`GGML_BACKEND_DL`), dibundel atau diunduh sesuai anggaran installer S4.
7. **BiRefNet: memori, bukan kecepatan, yang menjadi masalah** (§3.3). fp32 di DirectML lambat (2,7 dtk) karena VRAM meluap, bukan karena DirectML lambat.
8. **espeak-ng tidak dibutuhkan; sherpa-onnx dihapus.** Kokoro cukup dengan ORT (`input_ids`, `style [1,256]`, `speed`), dan ORT memang sudah kita bawa. G2P sendiri menggantikan espeak-ng. Ini menutup masalah terbuka di 05 §4 untuk bahasa Inggris.
9. **TTS bahasa Indonesia belum layak produksi** (WER bolak-balik ≥ 24%, beraksen Spanyol). Perbaikan murah: deteksi kata asing dan G2P campuran, jeda sesudah tanda baca. Perbaikan nyata butuh suara berbahasa Indonesia (05 §4 opsi 3).
10. **Telemetri ORT aktif secara default** di build Microsoft. Kinora mematikannya (`with_telemetry(false)`) dan harus menjaganya lewat uji.
11. **Lebih banyak thread tidak selalu lebih cepat:** Kokoro di CPU lebih cepat pada 4 thread daripada 8. Jumlah thread perlu dikalibrasi per model (Fase 1, default konservatif).
12. **Data uji FLEURS punya kalimat berulang** (dua penutur, satu teks), jadi WER Indonesia ditimbang oleh ±20 kalimat unik. Itu cukup untuk keputusan model, bukan untuk benchmark publik.

## 5. Rekomendasi & tindak lanjut
- **Fase 1, caption:** whisper.cpp + Vulkan, default turbo bila GPU; pemilihan model otomatis per perangkat dengan estimasi waktu; timestamp DTW token sebelumnya + snap VAD.
- **Fase 1, TTS:** Kokoro uint8 (DirectML, fallback CPU 4 thread). Bahasa Inggris siap; bahasa Indonesia berlabel "beta" dengan suara `ef_dora`/`em_alex`.
- **S3b (baru, sebelum fitur "hapus latar"):** cari konfigurasi cutout dengan **≤ 3 GB VRAM dan ≤ 2 GB RAM di CPU**, lalu bandingkan kualitas pada gambar uji yang sama. Kandidat:
  - export ulang BiRefNet pada 512/768 atau tanpa deform-conv `GatherND`;
  - IS-Net;
  - MODNet, hanya bila lisensi bobot terverifikasi.
- **S4:** VC++ runtime app-local, `DirectML.dll`, dan DLL backend Vulkan masuk anggaran installer (≤ 120 MB); uji di Sandbox **tanpa** toolchain terpetakan.
- **P0-06:** signing sebelum S4, karena SAC (temuan 1).

## 6. Belum diuji / tertunda
- iGPU Intel/AMD (DirectML dan Vulkan), GPU 8 GB, dan laptop 8 GB RAM. Perangkat belum ada, sama seperti S1/S2.
- Timestamp kata pada ucapan nyata dan bahasa Indonesia (butuh data dengan waktu kata).
- Dataset suara Indonesia berlisensi permisif untuk TTS.
- Perbandingan baseline "dengan espeak-ng" tidak dijalankan: espeak-ng adalah GPL-3.0 dan tidak akan dikirim, dan alternatif tanpa espeak sudah memenuhi kriteria untuk bahasa Inggris.

## 7. Reproduksi
Lihat [`spikes/s3-ai/README.md`](../../spikes/s3-ai/README.md). Rangkaian sesi yang menghasilkan laporan ini:
1. `models;bench vad,cutout`
2. `bench asr;bench tts` (CPU AVX2)
3. `bench asr` (Vulkan)
4. `bench vad;bench cutout;bench asr;bench tts --speed-only --tag mem;bench asr,tts,cutout --threads 4 --cpu-only --speed-only --tag lowspec`
5. enam sesi `bench cutout --model … --device …` (per konfigurasi)
