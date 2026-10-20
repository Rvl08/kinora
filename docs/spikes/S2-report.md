# Laporan Spike S2 — Media: Decode, Encode & Ekspor

> **Status:** Selesai (20 Okt 2026), diuji pada NVIDIA RTX 5070 Ti; QSV (Intel) dan AMF (AMD) **belum** (tidak ada perangkat).
> **ADR terkait:** [ADR-0005](../adr/0005-codec-and-ffmpeg-policy.md) · **Kode:** [`spikes/s2-media/`](../../spikes/s2-media/README.md) · **Lanjutan:** P0-05b
> **Rekomendasi:** ✅ Jalur ekspor **D3D11VA → (composite wgpu) → encoder hardware**, dengan fallback software **SVT-AV1 / OpenH264 (DLL Cisco)**. Semua kriteria S2 terpenuhi kecuali ukuran DLL, yang harus diselesaikan oleh build terpangkas P0-05b.

## 1. Ringkasan

| Kriteria (04 §4 S2) | Target | Hasil | Status |
|---|---|---|---|
| Ekspor 1080p H.264, encoder HW | ≥ 2× realtime | **12,2×** (NVENC, 364 fps); ekspor penuh + mix audio AAC **12,8×** | ✅ |
| Ekspor 4K HEVC, encoder HW | ≥ 1× realtime | **2,3×** dari sumber H.264 8-bit; **1,2×** dari sumber HEVC 10-bit (tetap 10-bit) | ✅ |
| DLL FFmpeg terkompresi | ≤ 60 MB | ±66 MB (build BtbN dev, 44 library) | ❌ → P0-05b |
| Lisensi build | tanpa `--enable-gpl` | LGPL v3, terverifikasi (P0-05); filter GPL seperti `eq` memang tidak ada | ✅ |
| Kebenaran warna | tag & kedalaman bit terjaga | BT.709/limited terbawa; HEVC/AV1 10-bit → Main10 | ✅ |
| OpenH264 saat runtime dari DLL Cisco | dapat dimuat tanpa merusak FFmpeg | Lewat crate `openh264` (`libloading`), di luar FFmpeg | ✅ |

## 2. Lingkungan & metode
- **Mesin:** i7-14700F (28 thread), RTX 5070 Ti (NVENC/NVDEC generasi Blackwell), Windows 11, FFmpeg **8.1.3 LGPL** (dev build P0-05), `ffmpeg-next` 8.1.0, wgpu 30.
- **Media kecepatan:** H.264 1080p30 (60 s), H.264 4K60 8-bit (20 s, "HP"), HEVC Main10 4K60 (60 s), ProRes 4K30.
- **Media kualitas:** referensi **lossless FFV1** 1080p30, 300 frame: *grafis/teks* (`testsrc2` + nomor frame) dan *mirip alami* (gradien halus + film grain temporal, sangat sulit dikompresi).
  Pembuatannya: `testsrc2`/`gradients` + `noise` + `hue`, `-c:v ffv1 -level 3`, bertag BT.709 limited.
- **Kualitas:** VMAF v0.6.1 + PSNR-Y + SSIM via `libvmaf`, frame dipasangkan **berdasarkan indeks** (lihat §5, temuan 3).
- **CPU:** waktu CPU proses ÷ waktu dinding (100% = satu core penuh).
- Laporan mentah: `tests/media/s2-out/report.json` (tidak di-commit; dapat dibangkitkan ulang).

## 3. Hasil

### 3.1 Ketersediaan encoder & waktu deteksi saat start
| Encoder | Terbuka? | Waktu buka | Catatan |
|---|---|---|---|
| `h264_nvenc` / `hevc_nvenc` / `av1_nvenc` | ✅ | 100–154 ms | |
| `h264/hevc/av1_qsv` | ❌ | 8–18 ms | Tidak ada GPU Intel (MFX session gagal) — **belum teruji** |
| `h264/hevc/av1_amf` | ❌ | < 1 ms | Tidak ada GPU AMD (`amfrt64.dll`) — **belum teruji** |
| `libsvtav1` | ✅ | 98 ms | |
| `libopenh264`, `prores_ks` | ✅ | ±1 ms | |
| `libvpx-vp9` | ✅ | 21 ms | |
| `dnxhd` | ✅ | 338 ms | inisialisasi tabel |

**Total ±1,0 detik.** Deteksi kemampuan **wajib** berjalan di thread latar belakang saat start (ADR-0005 §6), dan hasilnya di-cache per versi driver.

### 3.2 Kecepatan ekspor 1080p30 (decode D3D11VA, 600 frame)
| Encoder | fps | × realtime | CPU | Bitrate (target 8) |
|---|---|---|---|---|
| `hevc_nvenc` | 401 | 13,4× | 112% | 8,30 |
| `av1_nvenc` | 393 | 13,1× | 119% | 8,28 |
| `h264_nvenc` | 364 | 12,2× | 87% | 8,25 |
| `dnxhd` (DNxHR HQ) | 313 | 10,4× | 301% | 220 (intermediate) |
| `libsvtav1` (preset 8) | 139 | 4,6× | 1.035% | 9,27 |
| `libopenh264` (statis, FFmpeg) | 124 | 4,1× | 105% | 7,97 |
| `prores_ks` (422) | 71 | 2,4× | 1.168% | 69 (intermediate) |
| `libvpx-vp9` (good, cpu-used 5) | 15 | **0,49×** | 190% | 5,41 |

### 3.3 Kecepatan ekspor 4K60
| Sumber → encoder | fps | × realtime | Format output |
|---|---|---|---|
| H.264 8-bit → `hevc_nvenc` | 139 | 2,31× | HEVC Main, BT.709 |
| H.264 8-bit → `av1_nvenc` | 133 | 2,21× | AV1 Main |
| H.264 8-bit → `h264_nvenc` | 100 | 1,67× | H.264 Main |
| H.264 8-bit → `libsvtav1` (preset 10) | 47 | 0,78× | AV1 |
| HEVC 10-bit → `hevc_nvenc` | 72 | 1,20× | **HEVC Main10 (p010)** |
| HEVC 10-bit → `av1_nvenc` | 72 | 1,19× | **AV1 10-bit** |
| HEVC 10-bit → `h264_nvenc` | 61 | 1,01× | H.264 8-bit (down-convert) |
| HEVC 10-bit → `libsvtav1` | 32 | 0,54× | AV1 10-bit |

### 3.4 Bentuk pipeline (NVENC H.264)
| Mode | 1080p30: fps / CPU | 4K60 8-bit: fps / CPU |
|---|---|---|
| Decode software → encode | 362 / 267% | 100 / 210% |
| D3D11VA → unduh ke RAM → encode | 371 / 95% | 100 / 58% |
| **D3D11VA → zero-copy → NVENC** | **397 / 57%** | **102 / 18%** |
| D3D11VA → **composite wgpu** (YUV→RGB, efek WGSL S1, RGB→YUV) → encode | 296 / 93% | 98 / 93% |
| CUDA (NVDEC) | tidak aktif lewat API — lihat §7 | — |

- Pada 4K, semua mode **dibatasi encoder** (±100 fps). Composite GPU hanya memakan ±2% throughput di 4K dan ±20% di 1080p, padahal versinya masih **sinkron per frame** (tanpa pipelining).
- Decode hardware tidak mempercepat ekspor (encode-bound), tetapi **menurunkan CPU 3–4×**. Pada laptop, itu berarti lebih dingin dan lebih hemat baterai.
- Zero-copy adalah jalur paling efisien untuk ekspor **tanpa efek**. Ekspor dengan efek memakai composite; interop tekstur wgpu↔D3D11 (tanpa readback) adalah optimasi Fase 1+.

### 3.5 Kualitas pada bitrate setara (1080p30, target 6 Mbps)
| Encoder | Grafis: VMAF (min) / PSNR-Y / Mbps | Alami (grain): VMAF / Mbps | fps |
|---|---|---|---|
| `av1_nvenc` | **97,99** (96,5) / 46,5 / 6,34 | 66,7 / 6,36 | 437 |
| `libsvtav1` | 97,03 (92,8) / 45,5 / 7,39 | 66,6 / 4,68 | 120 |
| `hevc_nvenc` | 96,57 (94,8) / 45,2 / 6,47 | 68,0 / 6,28 | 422 |
| `h264_nvenc` | 96,39 (93,7) / 44,4 / 6,39 | 68,2 / 5,84 | 342 |
| `h264_nvenc` preset p7 + multipass | 96,66 (44,7*) / 44,8 / 6,22 | 68,0 / 6,25 | 227 |
| `libvpx-vp9` | 96,02 (93,1) / 42,5 / 4,29 | 68,9 / 16,3† | 25 |
| `libopenh264` (FFmpeg) | 93,81 (88,8) / 41,5 / 5,96 | 69,0 / 6,00 | 357 |
| **OpenH264 DLL Cisco** | 91,18 (28,3*) / 40,9 / 5,96 | 68,8 / 6,00 | 247 |

\* satu frame anjlok (min VMAF) · † VBR VP9 tanpa VBV melampaui target pada film grain

- **Konten grafis/teks** (mewakili screen recording, tutorial, judul): semua encoder hardware ≥ 96 VMAF pada 6 Mbps. AV1 NVENC terbaik. Preset p7 hanya +0,3 VMAF dengan kecepatan −35%, jadi **p5 adalah default yang tepat**.
- **Film grain**: semua encoder ±66–69 VMAF pada 6 Mbps. Konten berbutir butuh bitrate jauh lebih tinggi; preset ekspor "kualitas tinggi" perlu menaikkan bitrate (atau CRF/CQ) alih-alih mengganti encoder.
- **OpenH264** (fallback paten-aman tanpa GPU) layak tetapi paling lemah (−5 VMAF pada grafis, satu frame anjlok). Ia hanya fallback terakhir; untuk mesin tanpa encoder H.264 hardware, SVT-AV1 software lebih baik bila target platform menerima AV1.

### 3.6 Proxy (4K → 1080p H.264, GOP 10, tanpa B-frame, NVENC)
| Sumber | fps | × realtime |
|---|---|---|
| ProRes 422 4K30 (decode software) | 190 | 6,3× |
| H.264 4K60 8-bit (D3D11VA) | 158 | 2,6× |
| HEVC 10-bit 4K60 (D3D11VA, p010 → nv12 + skala di CPU) | 94 | 1,6× |

Klip 4K60 satu menit siap sebagai proxy dalam ±23–38 detik di latar belakang. Bottleneck sumber 10-bit adalah skala/konversi di CPU; `scale_d3d11` atau composite wgpu dapat memindahkannya ke GPU.

### 3.7 Thumbnail / filmstrip (60 thumbnail 160×90)
| Sumber | Mode keyframe (±1 s) | Mode tepat |
|---|---|---|
| H.264 1080p | 24,7 ms/thumb | 28,8 ms/thumb |
| HEVC 10-bit 4K | 63,8 ms/thumb | **277 ms/thumb** (16,6 s total) |

Strategi produksi: filmstrip awal pakai mode keyframe (cepat), diperhalus di latar belakang, dan diambil dari **proxy** untuk sumber 4K.

### 3.8 Ekspor lengkap dengan audio
- H.264 1080p 60 s + **mix audio** (audio asli 48 kHz × 0,8 + musik **44,1 kHz** × 0,4 → resample → AAC 192 kbps): **12,8× realtime**. Decode + resample + mix audio 0,09 s, encode AAC 0,69 s.
- Hasil: video 1.800 frame (59,97 s) + audio 2.814 frame AAC (60,00 s), **keduanya mulai di 0,000 s**, durasi selisih 1 frame. Tag BT.709 terbawa.

## 4. OpenH264 tanpa menautkan FFmpeg ke OpenH264
- Lisensi paten Cisco hanya mencakup **biner buatan Cisco**. `libopenh264` statis di build BtbN **tidak** tercakup (pemblokir rilis P0-05).
- Solusi yang terbukti: crate Rust `openh264` 0.9.9 (BSD-2) dengan fitur `libloading` **memuat `openh264-2.6.0-win64.dll` resmi saat runtime**. FFmpeg rilis sama sekali tidak perlu `--enable-libopenh264`.
- Verifikasi DLL tiga lapis (wajib juga di installer/pengunduh produksi):
  1. MD5 = `ce1282f5845f56761954282e4992730d` (file `.signed.md5.txt` dari Cisco),
  2. SHA-256 = `2076cb5675ec6c1a4c70e7a2a322552f547b6eeed649d6dfcd9e02a543b24691` (allow-list crate; crate menolak DLL lain),
  3. **Authenticode valid**, penanda tangan *Cisco WebEx LLC* (DigiCert). Sertifikatnya sudah kedaluwarsa (Feb 2026) tetapi **ber-timestamp**, sehingga tetap sah. Verifikasi produksi harus memeriksa status + timestamp, bukan tanggal kedaluwarsa sertifikat.
- DLL 956 KB, dimuat dalam milidetik.
- **Batasan:** Cisco 2.6.0 menolak `UsageType::CameraVideoNonRealTime` (dan non-real-time lainnya). OpenH264 juga tidak bisa menjaga bitrate tanpa *frame skip*, padahal ekspor tidak boleh membuang frame. Fallback ini sebaiknya memakai mode kualitas, bukan target bitrate ketat.

## 5. Temuan selama spike (pelajaran untuk kode produksi)
1. **Reset `pict_type` dan flag key sebelum encode.** Frame hasil decode membawa jenis gambarnya, dan encoder FFmpeg menganggap `pict_type = I` sebagai *paksa keyframe*. Sumber all-intra (FFV1, ProRes, gambar) menghasilkan output all-intra: encoder software overshoot **3–47×** dan NVENC kehilangan kualitas demi VBV. CLI FFmpeg mereset field ini; Kinora wajib melakukannya juga.
2. **Time base encoder = 1/fps, timestamp dan *durasi* frame di-rescale.**
3. **VMAF harus memasangkan frame berdasarkan indeks.** Timestamp milidetik dari Matroska (0, 33, 67 ms) vs 1/30 (0, 33,33, 66,67 ms) membuat setiap frame ke-3 dipasangkan salah, sehingga VMAF turun dari 96 ke 75 tanpa ada penurunan kualitas nyata.
4. **`frame.clone()` di ffmpeg-next = salinan piksel penuh.** Untuk frame GPU hasilnya frame tak valid (EINVAL di NVENC). Gunakan `av_frame_ref`.
5. **Pilih format hardware secara eksplisit** lewat callback `get_format` (seperti CLI). Ambil `hw_frames_ctx` encoder dari **frame**, bukan dari konteks decoder.
6. **SVT-AV1 menolak `maxrate` di luar mode CRF.** VBV hanya untuk encoder hardware.
7. **Thumbnail mode keyframe perlu flush** di dekat akhir file, karena decoder multi-thread menahan output.
8. **Filter GPL tidak tersedia di build LGPL** (`eq` terkonfirmasi tidak ada). Daftar filter produksi divalidasi otomatis (§6).

## 6. Komponen FFmpeg untuk build rilis (input P0-05b)
Daftar lengkap yang dapat dibaca mesin: [`scripts/ffmpeg/release-components.json`](../../scripts/ffmpeg/release-components.json). Validator [`scripts/ffmpeg/check-components.py`](../../scripts/ffmpeg/check-components.py) memastikan **133 nama** komponen benar-benar ada di FFmpeg 8.1 (sudah lulus; validator ini sempat menangkap `mpegps` → nama sebenarnya `mpeg`).

| Grup | Isi (ringkas) | Alasan |
|---|---|---|
| Decoder video | h264, hevc, av1 (+ libdav1d), vp8/vp9, mpeg2video, mpeg4, mjpeg, prores, dnxhd, ffv1, png, gif, webp, bmp, tiff, rawvideo | format kamera/HP/editor/gambar umum |
| Decoder audio | aac, mp3, mp2, opus, vorbis, flac, alac, ac3, eac3, pcm_* | rekaman, musik, film |
| Subtitle | subrip, ass, webvtt, mov_text (decode & encode) | impor/ekspor caption |
| Encoder | NVENC/QSV/AMF (h264/hevc/av1), libsvtav1, libvpx-vp9, prores_ks, dnxhd, png, mjpeg, gif, aac, libopus, flac, pcm, libmp3lame | ekspor & intermediate |
| Container | mov/mp4, matroska/webm, avi, mpegts, mpeg (PS), flv, ogg, wav/w64/aiff, mp3/aac/flac, image2, gif + muxer setara | impor/ekspor |
| Filter | hanya grafik minimal + scale, aresample, amix, volume, loudnorm, ebur128, transpose/flip, crop, pad, hwupload/download | efek visual dikerjakan oleh compositor WGSL, bukan FFmpeg |
| Protokol | file, pipe | **tanpa jaringan** di FFmpeg (privasi; unduhan lewat Rust) |
| Library eksternal | **pertahankan:** libdav1d, libsvtav1, libvpx, libopus, libmp3lame, libvpl, ffnvcodec, AMF, zlib · **opsional:** libwebp, libjxl, libzimg (HDR) · **buang 30+:** termasuk libzvbi (GPL), libopenh264 (DLL Cisco), libaom, libplacebo/shaderc, libass, freetype/harfbuzz, librsvg, libxml2, jaringan | ukuran & lisensi |

Ukuran build terpangkas **belum terukur**: build membutuhkan Docker/MSYS2 yang tidak tersedia di mesin dev, jadi akan diukur di CI P0-05b dengan target ≤ 30 MB terkompresi. Sumber ukuran terbesar saat ini (`avcodec` 87 MB) didominasi encoder/decoder eksternal yang dibuang (aom, rav1e, vvenc, kvazaar, openh264, dll.).

## 7. Yang belum terbukti (unknowns)
- **QSV & AMF:** tidak teruji (tidak ada perangkat). Wajib diuji di laptop Intel dan mesin AMD sebelum rilis, dengan kode yang sama (probe + matriks).
- **Decode CUDA/NVDEC lewat API:** lewat CLI berhasil (zero-copy CUDA juga), tetapi lewat kode spike tetap jatuh ke software meski sudah memakai `get_format`. Akar belum diselidiki. Tidak memblokir karena D3D11VA (netral-vendor) bekerja penuh.
- **Ukuran build rilis** (§6) dan **delay-load** tidak lagi relevan untuk OpenH264, tetapi tetap perlu dicek untuk AMF/QSV runtime (DLL vendor dimuat dinamis oleh FFmpeg).
- **Konten kamera nyata:** semua sumber sintetis. Uji kualitas perlu diulang dengan footage HP nyata berlisensi bebas (P0-11).
- **HDR (PQ/HLG):** HEVC 10-bit yang diuji adalah SDR; tone-mapping & metadata HDR belum disentuh.
- **Batas sesi NVENC** pada GPU konsumen (ekspor batch paralel) belum diukur.

## 8. Rekomendasi
1. **Jalur ekspor produksi:** D3D11VA decode → composite wgpu (bila ada efek/teks/transisi) → encoder hardware (NVENC/QSV/AMF dari hasil probe) → mux. Tanpa efek: **zero-copy**.
2. **Default encoder:** H.264 (kompatibilitas maksimum) via hardware; HEVC/AV1 hardware sebagai pilihan "ukuran kecil", 10-bit dipertahankan bila sumbernya 10-bit.
3. **Fallback tanpa GPU:** SVT-AV1 (kualitas sangat baik, multi-core) untuk AV1; OpenH264 dari DLL Cisco (diunduh saat dibutuhkan, diverifikasi tiga lapis) untuk H.264; VP9 hanya untuk WebM eksplisit, karena terlalu lambat sebagai default.
4. **Intermediate pro:** ProRes 422 dan DNxHR HQ (DNxHR 4× lebih cepat dan hemat CPU).
5. **Lanjutkan P0-05b** memakai `release-components.json`, dengan gerbang `ffmpeg:verify:ship` + validator komponen + pengukuran ukuran.
