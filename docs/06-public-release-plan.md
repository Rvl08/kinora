# 06 — Rencana Publikasi & Distribusi Tanpa Biaya

> Status: **diputuskan** (21 Okt 2026): kanal A + B tanpa biaya; docs/01 **tidak** dipublikasikan (salinan publik tanpa docs/01); nama tetap **Kinora** (cek cepat saja, atas keputusan pemilik).
> Latar belakang: pemilik ingin **tanpa biaya sama sekali**, termasuk tanpa membeli sertifikat. Dokumen ini menggantikan pilihan Certum di [ADR-0007](adr/0007-code-signing.md) §3 bila disetujui.

## 1. Bisakah tanpa membeli sertifikat? Ya, lewat dua kanal gratis

Smart App Control (SAC) dan SmartScreen hanya memercayai tanda tangan dari CA tepercaya. Dua cara gratis untuk mendapatkannya:

| | **Kanal A — Microsoft Store (MSIX)** | **Kanal B — GitHub Releases `setup.exe` + SignPath Foundation** |
|---|---|---|
| Biaya | Rp0. Pendaftaran developer perorangan gratis sejak Sept 2025 (verifikasi KTP/paspor + selfie) | Rp0. Program gratis untuk proyek open source |
| Yang menandatangani | Microsoft (paket di-re-sign setelah sertifikasi) | SignPath Foundation (OV) |
| Penerbit yang tampil | Nama penerbit di Store (bebas dipilih, tidak harus nama legal) | "SignPath Foundation" |
| SAC / SmartScreen | Lolos: aplikasi Store dipercaya | Bertanda tangan CA tepercaya. Reputasi SmartScreen dibangun bertahap |
| Instalasi | Satu klik dari Store, update otomatis; **tanpa wizard** | **Wizard NSIS** sesuai rencana (03 §6) |
| Syarat | Paket MSIX (Tauri tidak membuatnya langsung → `makeappx` / MSIX Packaging Tool, divalidasi di S4); lolos sertifikasi Store; URL kebijakan privasi | Repo **publik**, lisensi OSI ✓, sudah ada rilis, halaman "Code signing policy", MFA untuk semua anggota, **persetujuan manual** setiap rilis |
| Batasan penting | Hanya pengguna yang memakai Store | SignPath hanya menandatangani **artefak dari source kita sendiri** (lihat §2) |

**Usulan:** jalankan **keduanya**. Store melayani pengguna yang ingin "pasang dan lupa" (dan semua PC dengan SAC). `setup.exe` melayani pengguna yang ingin installer berwizard atau versi portabel.
Biaya total: **Rp0**. Certum (±€49/tahun) tidak diperlukan.

Sebelum SignPath menyetujui (butuh beberapa minggu dan rilis pertama), `setup.exe` dirilis sebagai **pratinjau tanpa tanda tangan**:
- SmartScreen akan memperingatkan, dan SAC akan memblokir;
- ini dijelaskan di halaman rilis, dengan Store sebagai alternatif.

## 2. Dampak arsitektur bila memakai SignPath (perlu persetujuan)

SignPath tidak mau menandatangani binari upstream (misalnya DLL FFmpeg), padahal SAC memeriksa **setiap** DLL. Karena itu isi `setup.exe` dibuat seminimal mungkin tanpa DLL tak bertanda tangan:

| Komponen | Sekarang (ADR-0005/0006) | Usulan |
|---|---|---|
| FFmpeg | DLL LGPL terpisah | **Ditautkan statis ke `kinora.exe`**, dibangun dari source di CI kita (P0-05b) |
| Backend ggml (Vulkan) | DLL terpisah (usulan S3) | Statis di `kinora.exe` (+16 MB terkompresi) atau DLL dari source kita sendiri (boleh ditandatangani SignPath) |
| ONNX Runtime | Statis | Tetap |
| `DirectML.dll` | Dibundel | **Diunduh saat pertama kali fitur AI dipakai** dari NuGet Microsoft (sudah bertanda tangan Microsoft; SHA-256 dipin), seperti OpenH264 |
| VC++ runtime | App-local | CRT statis (`+crt-static`) bila memungkinkan; jika tidak, runtime app-local (bertanda tangan Microsoft, "system library") |

- **LGPL dengan tautan statis:** sah selama pengguna dapat menautkan ulang dengan FFmpeg yang dimodifikasi. Karena seluruh source Kinora terbuka beserta skrip build-nya, syarat itu terpenuhi dengan sendirinya.
  - ADR-0005 §5 ("DLL terpisah dan dapat diganti") **diamandemen** untuk kanal B.
  - `THIRD_PARTY_NOTICES` menjelaskan cara build ulang dengan FFmpeg lain.
- **Belum pasti, ditanyakan saat mendaftar ke SignPath:**
  - apakah FFmpeg yang dikompilasi dari source di CI kita dan ditautkan statis ke exe kita dihitung sebagai "artefak dari source kita" (praktik umum: setiap aplikasi Rust menautkan crate pihak ketiga secara statis);
  - status `DirectML.dll` sebagai komponen proprietary (usulan di atas menghindarinya).

## 3. Audit kesiapan repo publik (21 Okt 2026)

| Pemeriksaan | Hasil |
|---|---|
| Identitas commit (semua cabang) | ✅ Hanya `Rvl08 <150601118+Rvl08@users.noreply.github.com>`, 9 commit, semua DCO |
| Email pribadi di riwayat / file | ✅ Tidak ada |
| Path host (`C:\Users\…`, `D:\Programming\…`) | ✅ Tidak ada; hanya `D:\Agent Skills\…` (docs/01) dan `D:\Ffmpeg\…` (catatan ADR-0005), tidak mengidentifikasi orang |
| Rahasia (kunci, token) | ✅ Tidak ada |
| File besar | ✅ Terbesar 390 KB (montase S3b, gambar public domain) |
| Media uji, model, vendor, build | ✅ Tidak di-commit (`.gitignore`) |
| **docs/01 — Laporan reverse engineering CapCut** | ⚠️ Isinya observasi: nama modul dan ukuran DLL, label fitur UI, dan string monetisasi (`m10n`) dari biner CapCut. Tidak ada kode, kunci, atau endpoint. **Tetapi** judul dan metodenya terang-terangan "reverse engineering" aplikasi proprietary yang EULA-nya melarang RE → **keputusan pemilik** (§4) |
| **Nama "Kinora"** | ⚠️ Belum dicek formal (P0-01). Pencarian cepat: tidak ada merek dagang yang ditemukan. Di GitHub ada organisasi `kinora-dev/kinora` (dashboard uji browser, 39★), dan "Kinora" juga nama penampil film 1896. Cek PDKI (Indonesia), WIPO Global Brand Database, dan USPTO/EUIPO sebelum nama dipesan di Store |

## 4. Keputusan yang dibutuhkan dari pemilik
1. **Jalur tanda tangan:** kanal A + B gratis (usulan), atau tetap Certum (berbayar).
2. **docs/01 di repo publik:**
   - (a) dipublikasikan apa adanya;
   - (b) **dikeluarkan dari repo publik**: riwayat ditulis ulang dengan `git filter-repo` di salinan publik, sedangkan repo lokal tetap utuh;
   - (c) ditulis ulang menjadi "analisis fitur kompetitor" yang hanya memakai informasi publik.
3. **Nama:** tetap "Kinora" (setelah cek merek) atau ganti sebelum publik. Nama sulit diganti setelah Store, SignPath, dan repo dibuat.

## 5. Langkah bertahap setelah keputusan
1. Siapkan file:
   - halaman **Kebijakan privasi** (Kinora offline; unduhan model hanya atas permintaan pengguna);
   - bagian **Code signing policy** di README (syarat SignPath);
   - field `repository` di `Cargo.toml`;
   - `THIRD_PARTY_NOTICES` awal.
2. Bila 2(b): buat salinan publik tanpa docs/01 lewat `git filter-repo`.
3. **Pemilik** membuat repo publik di github.com dengan MFA aktif. Setelah itu:
   - push pertama (memakai kredensial pemilik);
   - aktifkan branch protection, secret scanning, private vulnerability reporting, Dependabot, permission Actions read-only, dan environment `release` dengan reviewer.
4. CI berjalan untuk pertama kali (bukti P0-04), lalu P0-05b (FFmpeg rilis, statis bila B).
5. Rilis pratinjau pertama (tanpa tanda tangan) → daftar ke SignPath Foundation dan Microsoft Store (pesan nama).
6. Setelah SignPath disetujui: `KINORA_SIGN_MODE` diganti dengan GitHub Action SignPath di alur rilis. `verify.mjs` tetap menjadi gerbang rilis, dengan `publishers` = "SignPath Foundation".
