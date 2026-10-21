# ADR-0007 — Code signing

- **Status:** Accepted (21 Okt 2026). Penyedia: **Certum Open Source Code Signing (cloud)**, diputuskan pemilik proyek pada 21 Okt 2026 (§3) · **Terkait:** P0-06, S4, [ADR-0006](0006-local-ai-runtime.md) §9

## Konteks
- **Smart App Control (SAC)** aktif di mesin dev dan di banyak PC Windows 11 baru.
  - SAC memeriksa **setiap exe dan dll** yang dimuat.
  - Bila layanan reputasi tidak bisa memutuskan, SAC hanya mengizinkan file yang ditandatangani sertifikat dari CA dalam **Microsoft Trusted Root Program**.
  - Sertifikat self-signed tidak dihitung, dan satu DLL tak bertanda tangan cukup untuk merusak aplikasi (S3 temuan 1).
- **SmartScreen** memperlakukan OV dan EV sama sejak 2024: reputasi dibangun per penerbit, lintas rilis.
- **Aturan CA/Browser Forum:** sejak Juni 2023 kunci privat code signing wajib berada di HSM, token, atau HSM cloud. File PFX hanya untuk uji.
- **Isi rilis Kinora** (inventaris 21 Okt 2026):

  | Komponen | Status tanda tangan |
  |---|---|
  | `kinora.exe`, installer, uninstaller, DLL backend ggml | Milik kita, belum ditandatangani |
  | DLL FFmpeg (build sendiri, P0-05b) | **Tidak bertanda tangan** dari upstream |
  | `DirectML.dll` | Bertanda tangan Microsoft |
  | VC++ runtime | Bertanda tangan Microsoft |
  | Bootstrapper WebView2 | Bertanda tangan Microsoft |
  | OpenH264 | Bertanda tangan Cisco, diunduh saat runtime |

- **Pemilik proyek** adalah pengembang perorangan di Indonesia.
  - **Azure Artifact Signing tidak tersedia**: perorangan hanya di AS/Kanada; organisasi di AS, Kanada, UE, dan Inggris.

## Keputusan
1. **Semua file PE yang dikirim bertanda tangan Authenticode**, dengan digest file SHA-256 dan timestamp RFC 3161 SHA-256.
   - Cakupan: installer, uninstaller, exe, DLL FFmpeg, dan DLL backend.
   - Binari pihak ketiga yang sudah ditandatangani vendornya (Microsoft, Cisco) dikirim **apa adanya**; tanda tangannya tidak pernah ditimpa.
   - Daftar yang diizinkan ada di [`scripts/signing/policy.json`](../../scripts/signing/policy.json).
2. **Pipeline tidak terikat penyedia:** [`scripts/signing/sign.mjs`](../../scripts/signing/sign.mjs) menandatangani lewat `signtool` dengan mode:
   - `store`: kunci di Windows certificate store, misalnya smartcard, Certum SimplySign, atau token OV;
   - `artifact-signing`: Azure, untuk masa depan bila memenuhi syarat;
   - `pfx`: khusus uji.

   Timestamp server dicoba berurutan. Skrip yang sama dipakai di dua tempat:
   - Tauri (`bundle.windows.signCommand`), untuk exe, installer, dan uninstaller;
   - langkah rilis untuk DLL.
3. **Penyedia: Certum Open Source Code Signing, versi cloud (SimplySign)**, atas nama pemilik proyek. Pemilik menyetujui bahwa nama legalnya tampil sebagai penerbit ("Open Source Developer, *nama*").
   - Pembelian dan verifikasi identitas dilakukan pemilik setelah repo publik tersedia.
   - Sesudah sertifikat terbit, subjeknya dimasukkan ke `policy.json` → `publishers`.
   - Mode `store` dipakai lewat SimplySign Desktop (lokal) atau runner CI (§5).
4. **Gerbang rilis:** `pnpm run sign:verify <folder rilis>` wajib lulus. Setiap PE harus:
   - milik kita: tanda tangan valid, SHA-256, dan bertimestamp; **atau**
   - pihak ketiga yang diizinkan: nama file dan penanda tangan cocok dengan aturan.

   File tak bertanda tangan, rusak, rantai tidak tepercaya, atau penanda tangan tak dikenal menggagalkan rilis.
5. **Kunci:** tidak pernah ada di repo atau runner dalam bentuk file.
   - Di CI hanya kredensial akses HSM cloud (misalnya TOTP SimplySign), disimpan di GitHub Environment `release` dengan persetujuan manual.
   - Rilis hanya dibuat dari tag yang ditandatangani.
6. **Build dev di mesin dengan SAC tetap di Windows Sandbox.** Build script Cargo dibuat ulang setiap build dan tidak praktis ditandatangani. Sertifikat menyelesaikan distribusi dan menjalankan biner uji di host, bukan `cargo build`.

## Pilihan penyedia (untuk diputuskan)

| Opsi | Biaya | Penerbit yang tampil | Bisa menandatangani DLL FFmpeg kita? | CI | Catatan |
|---|---|---|---|---|---|
| **A. Certum Open Source Code Signing (cloud, SimplySign)** | ±€49/tahun | "Open Source Developer, *nama legal*" | ✅ | Bisa (TOTP; alat komunitas, perlu diaudit) | RSA ≥ 3072. Verifikasi identitas perorangan. **Nama legal pemilik tampil publik** di setiap file |
| B. SignPath Foundation | Gratis | "SignPath Foundation" | ❌ Hanya artefak dari source kita sendiri; library upstream harus punya tanda tangan sendiri | Bawaan (GitHub Action), setiap rilis disetujui manual | Syarat: repo publik, lisensi OSI, sudah ada rilis, halaman "Code signing policy". Komponen proprietary dilarang kecuali system library (`DirectML.dll` perlu dikonfirmasi). DLL FFmpeg tak bertanda tangan berisiko diblokir SAC |
| C. Sertifikat OV komersial perorangan (Sectigo, SSL.com eSigner, dll.) | ±$150–300/tahun (+ biaya cloud signing) | Nama legal | ✅ | Bisa (eSigner/KMS) | Tanpa batasan "open source" |
| D. Azure Artifact Signing | $9,99/bulan | Nama legal/organisasi | ✅ | Terbaik | **Tidak memenuhi syarat** (perorangan di Indonesia) |
| E. Microsoft Store (MSIX) | Gratis | Microsoft re-sign | ✅ (paket MSIX) | — | Kanal tambahan, bukan pengganti `setup.exe` (03 §6) |

**Rekomendasi: A (Certum Open Source, versi cloud)** untuk rilis pertama. **Dipilih** (keputusan 3).
- Murah, bisa menandatangani seluruh isi rilis termasuk FFmpeg, dan dapat diotomasi.
- Konsekuensi yang harus disetujui pemilik: **nama legal tampil** sebagai penerbit (berbeda dengan email noreply yang dipilih untuk commit).
- Bila itu tidak diinginkan, B (SignPath) memberi anonimitas, tetapi DLL FFmpeg tidak bisa ditandatangani. Itu harus diuji dulu terhadap SAC sebelum dipilih.
- Sertifikat Certum OSS hanya untuk software open source. Bila Kinora kelak berbayar atau komersial, beralih ke C.

**Prasyarat umum:**
- Repositori GitHub **publik** dengan lisensi dan halaman proyek (Certum dan SignPath memverifikasi proyek open source). Ini juga membuka P0-04 (CI) dan P0-05b.
- Akun dengan MFA.

## Konsekuensi
- ➕ Satu skrip untuk semua penyedia; mengganti penyedia hanya mengubah variabel lingkungan dan `policy.json` (`publishers`).
- ➕ Gerbang `sign:verify` mencegah rilis dengan DLL tak bertanda tangan, yang merupakan kegagalan khas di mesin dengan SAC.
- ➖ Biaya tahunan (A atau C) atau ketergantungan pada persetujuan pihak ketiga (B).
- ➖ Reputasi SmartScreen tetap mulai dari nol; peringatan awal tetap mungkin muncul walau sudah ditandatangani.
- ➖ Alat CI untuk Certum cloud adalah proyek komunitas → kodenya harus dipin ke commit dan diaudit, atau CI dijalankan di runner self-hosted dengan SimplySign Desktop resmi.

## Verifikasi
Self-test end-to-end di Windows Sandbox ([`scripts/signing/sandbox/`](../../scripts/signing/sandbox/)), 21 Okt 2026: **12/12 lulus**.
- Sertifikat uji RSA-3072 dibuat dan dipercaya **hanya di dalam VM**.
- Muatan berbentuk rilis: exe kita, 7 DLL FFmpeg + `ffprobe.exe`, `DirectML.dll` (Microsoft), dan OpenH264 (Cisco).

| Uji | Hasil |
|---|---|
| Verifikasi sebelum ditandatangani | Gagal (benar: 9 file tak bertanda tangan) |
| Mode `store` | 9 file ditandatangani, 2 file pihak ketiga dilewati, timestamp DigiCert RSA4096 |
| Verifikasi sesudahnya | Lulus 11/11 |
| Jalankan ulang | Tidak melakukan apa-apa |
| `ffprobe` yang sudah ditandatangani | Tetap berjalan |
| Tanpa `--allow-test` | Penerbit uji ditolak (gerbang kebijakan bekerja) |
| 1 byte diubah | `HashMismatch` → ditolak |
| Mode `pfx` | Lulus |
| File bertanda tangan pihak lain | Tidak ditimpa (`refuse`) |

Belum diuji, karena butuh sertifikat asli:
- perilaku SAC di host terhadap biner bertanda tangan;
- mode `store` dengan SimplySign;
- integrasi `signCommand` di bundler Tauri (S4).
