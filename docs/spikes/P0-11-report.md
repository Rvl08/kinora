# Laporan P0-11 — Fixture Media Uji

> **Status:** Selesai (22 Okt 2026). Semua 15 fixture diunduh atau dibangkitkan di mesin dev (Windows 11, RTX 5070 Ti, FFmpeg `n8.1.3-18-g46d899d7a0`). Semuanya lolos `verify.mjs`.
> **Kode:** [`scripts/fixtures/`](../../scripts/fixtures/README.md) · **Kredit:** [`ATTRIBUTION.md`](../../scripts/fixtures/ATTRIBUTION.md) · **Rencana:** [04 §6](../04-phase0-plan.md)
> **Hasil:** semua butir §6 punya fixture berlisensi bebas. Total 305 MB di disk, di bawah target 1 GB.
> ⚠️ Celah:
> (1) kata pengisi ("eee", "anu") di rekaman podcast belum dianotasi;
> (2) HEIC dibuat sendiri, bukan hasil kamera ponsel;
> (3) klip HDR10 butuh NVENC, jadi runner CI tanpa GPU NVIDIA melewatinya.

## 1. Ringkasan

| Fixture | Butir §6 | Sumber · lisensi | Ukuran | Terverifikasi |
|---|---|---|---|---|
| `video-h264-1080p30-vfr` | 1080p30 H.264 HP, VFR | Dibangkitkan (`h264_nvenc`) · MIT/Apache | 22,3 MB | ✅ |
| `video-hevc-2160p60-hdr10` | 4K60 HEVC 10-bit HDR | Dibangkitkan (`hevc_nvenc`) · MIT/Apache | 52,8 MB | ✅ |
| `video-prores422-2160p` | 4K ProRes 422 | Dibangkitkan (`prores_ks`) · MIT/Apache | 113,2 MB | ✅ |
| `video-av1-1080p30` | 1080p AV1 | Dibangkitkan (`libsvtav1`) · MIT/Apache | 10,6 MB | ✅ |
| `video-vp9-alpha-1080p30` | WebM VP9 + alpha | Dibangkitkan (`libvpx-vp9`) · MIT/Apache | 4,9 MB | ✅ |
| `video-screen-1440p30-lossless` | Klip layar, teks tajam | Dibangkitkan (`ffv1`) · MIT/Apache | 49,8 MB | ✅ |
| `photo-jpeg-phone` | Foto JPEG | Wikimedia Commons "Mamões", ArionStar · CC0 | 3,2 MB | ✅ |
| `photo-jpeg-phone-rotated` | Foto JPEG (EXIF rotasi) | Wikimedia Commons "Golden Hill street sign", Chemicydro · CC0 | 4,0 MB | ✅ |
| `photo-heic-grid` | Foto HEIC | Dibangkitkan dari "Mamões" (`libkvazaar` + penulis HEIF sendiri) · CC0 | 1,2 MB | ✅ |
| `audio-id-podcast` | Podcast Indonesia dengan pengisi | Wikimedia Indonesia, *Bincang Mingguan GLAM Indonesia* ep. 14,5 · CC BY 3.0 | 4,4 MB | ✅ |
| `audio-id-interview` | Wawancara 2 pembicara | Antara TV Indonesia, *International Corner* (Daniel Simanjuntak) · CC BY 3.0 | 2,5 MB | ✅ |
| `audio-song-vocals` | Lagu dengan vokal | Josh Woodward, "Stickybee" · CC BY 3.0 US | 11,3 MB | ✅ |
| `audio-song-instrumental` | (referensi pemisahan vokal) | Josh Woodward, "Stickybee (No Vocals)" · CC BY 3.0 US | 4,2 MB | ✅ |
| `speech-id-fleurs-transcripts` | (data S3) | Google FLEURS `id_id` dev, transkrip · CC BY 4.0 | 0,2 MB | ✅ |
| `speech-id-fleurs-clips` | (data S3) | Google FLEURS `id_id` dev, 25 klip · CC BY 4.0 | 20,6 MB | ✅ |

**Ukuran dan waktu di mesin dev:**
- Di disk: **305 MB** (255 MB dibangkitkan, 50 MB unduhan).
- Transfer `fetch`: ±330 MB. Arsip FLEURS 220 MB hanya dipakai untuk mengekstrak 25 klip, lalu dihapus.
- `fetch`: 593 dtk, terutama arsip FLEURS (406 dtk) dan potongan 70 MiB dari Commons (132 dtk).
- `generate`: ±35 dtk dengan NVENC.
- `verify`: ±2 dtk.

## 2. Cara kerja

**Skrip** (Node ESM, gaya sama dengan `scripts/ffmpeg/`):

| File | Fungsi |
|---|---|
| `manifest.json` | Satu entri per fixture: `id`, `purpose`, `kind`, lisensi dan atribusi, sumber yang dipin atau resep FFmpeg persis, `expect` (properti ffprobe), hash file |
| `fetch.mjs` | Unduh hanya dari URL yang dipin; ukuran dan SHA-256 dicek, tidak cocok = ditolak dan dihapus |
| `generate.mjs` | Membangkitkan fixture sintetis dengan FFmpeg dev, memilih encoder pertama yang benar-benar jalan |
| `verify.mjs` | Tabel semua fixture; exit 1 jika ada yang hilang, rusak, atau propertinya salah |
| `lib.mjs`, `heif.mjs` | Fungsi bersama; penulis HEIF minimal |

**Unduhan:**
- Bisa dilanjutkan (resume dari `.part` lewat HTTP Range; sudah diuji dari byte 1.000.000).
- Mencoba ulang saat HTTP 429/5xx, menghormati `Retry-After`.
- User-Agent deskriptif untuk Wikimedia.
- Setiap unduhan masuk direktorinya sendiri di `.sources/<id>/` dan tidak pernah dieksekusi. File hanya dibaca oleh FFmpeg atau bsdtar.

**Potongan byte:** dua video Commons panjangnya 69 MB dan 121 MB. Yang diunduh dan dipin hanya **rentang byte awalnya** (16 MiB dan 70 MiB). Dari situ FFmpeg memotong audionya (`-c:a copy`, tanpa re-encode).

**Verifikasi dua lapis** (detail di README):
1. **SHA-256.**
   - Unduhan dan anggota arsip adalah byte persis: beda = gagal.
   - Keluaran FFmpeg dipin untuk satu build FFmpeg dan satu encoder (`pin`). `.state.json` lokal mencatat build dan encoder yang membuat tiap file.
   - Beda hash dihitung gagal hanya jika file dibuat dengan build dan encoder software yang sama dengan pin, atau file berubah setelah dibuat.
   - Selain itu statusnya `differs` (peringatan), dan pemeriksaan properti yang memutuskan.
   - Alasannya: keluaran encoder tidak dijamin identik antar-build FFmpeg, dan NVENC bisa berbeda antar-GPU/driver.
2. **Properti ffprobe**, selalu ditegakkan: container, codec, profile, resolusi, pix_fmt (bit depth), fps rata-rata, VFR, tag warna, metadata HDR10, alpha, rotasi EXIF, profil ICC, tile grid HEIF, durasi.

**Determinisme:** `generate --force` dua kali menghasilkan byte identik untuk ketujuh fixture (termasuk yang NVENC) di mesin ini. Uji negatif juga sudah dijalankan: satu byte ditambahkan ke file, `verify` gagal dengan `MISMATCH`, lalu `fetch` mengunduh ulang hanya file itu.

## 3. Isi tiap fixture

### 3.1 Video (dibangkitkan)

Semua klip video memakai `testsrc2` dengan nomor frame dan timestamp tercetak (resep S1), bertag BT.709 kecuali klip HDR, dan memakai beep 1 kHz di awal tiap detik untuk cek A/V sync.

- **H.264 VFR:**
  - High profile dengan B-frame, AAC 48 kHz.
  - Timestamp ber-jitter ±10 ms (timescale 90 kHz) seperti ponsel, potongan 15 fps pada detik 8–12 ("cahaya redup"), dan satu frame hilang tiap 23 frame.
  - Hasilnya 517 frame dalam 20 dtk, dengan durasi frame 20–150 ms.
- **HEVC HDR10:**
  - Main 10, 3840×2160, 60 fps, BT.2020/PQ.
  - Mastering display P3-D65 1000 nit dan MaxCLL/MaxFALL 1000/400 ada di container (`mdcv`/`clli`) dan di SEI tiap frame.
- **ProRes 422:** profil Standard, 4:2:2 10-bit, 23,976 fps (120 frame), PCM 24-bit, vendor `apl0`.
- **AV1:** Main, 1080p30, MP4 + AAC. Dibuat dengan SVT-AV1 (CPU), jadi tidak butuh GPU.
- **VP9 alpha:**
  - Cakram buram dengan tepi lembut 60 px bergerak di atas latar transparan, plus pita bawah 50% transparan.
  - Alpha diuji dengan decode `libvpx-vp9` dan komposit di atas magenta (diperiksa visual).
- **Klip layar:**
  - 2560×1440, FFV1 RGB lossless.
  - Isi: teks Apache License 20 px yang bergulir, teks 11 px dan 14 px, garis 1 px, teks merah di atas biru (uji chroma 4:2:0), kursor bergerak, dan bilah judul/status bergaya aplikasi.
  - Menjadi master untuk uji encoder dan scaling.

### 3.2 Foto

- **JPEG** (dua foto iPhone CC0 dari Commons, keduanya dengan profil ICC Display P3):
  - `photo-jpeg-phone`: iPhone 15 Pro, EXIF lengkap **termasuk GPS**. Berguna untuk menguji bahwa ekspor membuang lokasi.
  - `photo-jpeg-phone-rotated`: iPhone 11, **EXIF Orientation 6** (ffprobe: rotasi −90).
- **HEIC:**
  - Disusun seperti foto iPhone: item `grid` 4032×3024 dari 8×6 tile HEVC 512×512 (tepi kanan/bawah dipotong), `colr` nclx Display P3.
  - Diverifikasi dengan dua decoder: tile grid FFmpeg 8.1 dan **Microsoft HEIF Decoder** (WIC, 4032×3024).

### 3.3 Audio

- **Podcast Indonesia:**
  - Dua pembawa acara Wikimedia Indonesia mengobrol spontan tanpa naskah dalam siaran langsung (menit 0:46–6:46).
  - Whisper-small (dengan prompt pengisi) menandai ±24 token pengisi di menit 4–7. Angka sebenarnya lebih tinggi karena Whisper cenderung membuang disfluensi. Ada pula banyak jeda ragu dan pengulangan ("kalau, kalau teman-teman").
- **Wawancara:**
  - Wartawan Antara (Aria Cindyara) dan diplomat Daniel Simanjuntak, mikrofon studio, 3:30.
  - 19 dtk pertama adalah narasi pembuka oleh wartawan; sesudahnya giliran bicara bergantian (transkrip kasar Whisper sudah dicek).
- **Lagu:**
  - "Stickybee" (rock, vokal utama pria, 3:19), plus versi **tanpa vokal** resmi dari artisnya dengan durasi identik (198,97 dtk).
  - Pasangan ini memberi referensi kasar untuk pemisahan vokal. Keduanya di-encode terpisah ke Vorbis, jadi selisih per sampel bukan kebenaran absolut.
- **FLEURS:**
  - Klip dan transkrip yang sama dengan S3 (25 baris pertama `dev.tsv`, byte identik dengan `tests/media/s3/id`).
  - Kini dipusatkan dengan URL Hugging Face yang dipin ke commit `70bb2e84`.

## 4. Sumber dan lisensi

- **Kebijakan lisensi:**
  - Hanya CC0, CC BY, atau buatan sendiri.
  - Tidak ada NC, ND, GPL, atau aset CapCut/Concat.
  - Lisensi tiap file Commons dicek lewat API `extmetadata`, dan SHA-1 Commons dicocokkan dengan unduhan.
- **Atribusi:** semua kredit CC BY, termasuk catatan perubahan (potongan, audio saja), ada di `ATTRIBUTION.md`.
- **URL:**
  - Hugging Face dipin ke commit.
  - Commons memakai URL file asli. Isinya berubah hanya jika ada versi baru diunggah, dan hash akan menolaknya. Versi lama tetap tersedia di URL `archive/` Commons.
- **Penelusuran yang tidak dipakai:**

| Kandidat | Alasan tidak dipakai |
|---|---|
| Wawancara Indonesia di Commons (Univ. Indonesia, CC BY-SA 4.0) | ShareAlike di luar kebijakan |
| *Creative Commons Indonesia Podcast* (CC BY-SA 4.0, 67 MB) | ShareAlike di luar kebijakan |
| Common Voice | Ucapan dibaca, bukan spontan; unduhan butuh persetujuan syarat |
| Remix ccMixter | Rantai atribusi sampel rumit |
| File konformans HEIF Nokia | Lisensi Nokia, bukan CC0 |

## 5. Temuan teknis

1. **Metadata HDR10 tidak bisa diset dari CLI FFmpeg.**
   - Solusinya: satu frame "seed" AV1 dibuat dengan `libsvtav1 -svtav1-params mastering-display=…:content-light=…`, lalu di-`loop` di bawah gambar lewat `overlay`.
   - Side data frame ikut diteruskan ke `hevc_nvenc`, yang menulis SEI, dan ke muxer MP4 (`mdcv`/`clli`).
2. **VFR hilang tanpa `-enc_time_base:v 1:90000`.** FFmpeg 8 memakai 1/fps sebagai time base encoder, sehingga jitter timestamp dibulatkan ke grid 1/30.
3. **`drawtext` diam-diam tidak menggambar apa pun** jika teks berisi `%` (misalnya "50%" di teks lisensi). Untuk teks dari file perlu `expansion=none`.
4. **Build LGPL ini memuat `libkvazaar`** (encoder HEVC software, BSD-3-Clause, hanya 8-bit). Dengan itu HEIC bisa dibuat tanpa GPU. FFmpeg tidak punya muxer HEIF, jadi `heif.mjs` (±250 baris) menulis `ftyp/meta/iinf/iref/iprp/iloc/mdat` sendiri.
5. **Commons melayani HTTP Range** (206), jadi memotong awal rekaman panjang menghemat ±170 MB. Bandwidth upload.wikimedia.org di sini ±0,5 MB/dtk.

## 6. Celah dan risiko

| Celah / risiko | Dampak | Usulan |
|---|---|---|
| **Kata pengisi tidak dianotasi.** Tidak ditemukan podcast Indonesia spontan berlisensi CC0/CC BY yang padat "eee/anu" dan sudah ditranskrip. | Uji penghapusan pengisi belum punya kebenaran (ground truth) | Pemilik merekam sendiri (lihat di bawah tabel) atau menganotasi `audio-id-podcast` secara manual. Hasilnya bisa jadi fixture `generate`/`download` baru |
| "anu" jarang muncul di klip GLAM (yang dominan "eee", "jadi", "ya") | Cakupan pengisi khas Indonesia terbatas | Sama: rekaman sendiri |
| HEIC bukan buatan ponsel: tanpa thumbnail, EXIF, ICC, depth map, gain map HDR, atau rotasi `irot` | Kasus HEIC iPhone yang rumit belum teruji | Cari sampel CC0 dari ponsel pemilik (pemilik memotret dan merilisnya CC0) |
| Klip HDR10 hanya bisa dibuat dengan NVENC (build ini tidak punya HEVC 10-bit software) | CI tanpa GPU NVIDIA melewatinya | `generate --allow-skip` dan `verify --allow-missing` di CI; mesin dev tetap wajib lengkap |
| Video sintetis (`testsrc2`), bukan rekaman kamera | Statistik kompresi tidak mirip footage alami; tidak ada matriks rotasi video, HLG, atau Dolby Vision | Tambah klip kamera CC0 (mis. NASA, domain publik) jika uji kualitas membutuhkannya |
| Hash keluaran FFmpeg terikat build `n8.1.3-18-g46d899d7a0` | Setelah FFmpeg di-upgrade, status `differs` sampai dipin ulang | `generate --pin --force` dan `fetch --pin` bersamaan dengan perubahan `lock.json` |
| Font Consolas (Windows) dipakai untuk teks | `generate` hanya untuk Windows; di OS lain hash berbeda | Bisa diganti font bebas yang di-vendor jika CI non-Windows diperlukan |
| Lisensi CC BY dari YouTube bergantung pada tinjauan Commons; file GLAM masih berkategori *License review needed* (diunggah oleh/atas nama Wikimedia Indonesia sendiri) | Kecil, karena hanya dipakai untuk uji dan tidak didistribusikan | Cek ulang halaman Commons sebelum dipakai di luar pengujian |
| Unduhan dari Commons lambat (70 MiB ≈ 2 menit) | `fetch` pertama ±10 menit | Dapat diterima; resume menangani gangguan |

**Instruksi rekaman sendiri (podcast pengisi), jika pemilik setuju:**
1. Dua orang mengobrol bebas 5 menit dalam bahasa Indonesia sehari-hari tanpa naskah, sengaja tidak menyunting "eee", "anu", "apa namanya", dan "gitu".
2. Rekam dengan ponsel (48 kHz). Kirim WAV/M4A beserta pernyataan rilis CC0 dari kedua pembicara.
3. Tandai waktu setiap pengisi di Audacity (label track, ekspor TXT) sebagai kebenaran.
4. File diunggah ke rilis GitHub proyek (gratis), lalu dipin di `manifest.json` seperti unduhan lain.
