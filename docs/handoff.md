# Catatan Serah Terima Antarsesi

Dokumen ini menjembatani satu sesi kerja ke sesi berikutnya: apa yang baru selesai, apa yang belum diverifikasi, dan
apa langkah pertama berikutnya. Sumber kebenaran progres tetap tabel [04](04-phase0-plan.md) §3.1; aturan kerja ada
di `CLAUDE.md`. Perbarui dokumen ini di akhir setiap sesi.

**Terakhir diperbarui:** 24 Okt 2026, sesi Claude Desktop (Windows). Riwayat sesi cloud 22 Okt ada di §1–§5 di bawah;
pembaruan 24 Okt ada di §0.

## 0. Pembaruan 24 Okt 2026 (Claude Desktop, Windows)

**Temuan penting: Smart App Control kini aktif penuh di Windows Sandbox.**
- Nilai `VerifiedAndReputablePolicyState` adalah 1 (21–22 Okt masih 2).
- Code Integrity memblokir `cargo.exe` toolchain 1.99.1 yang dipin, sehingga build script, biner tes, `bash.exe` MSYS2,
  dan probe configure ikut terblokir (log: `tests/sandbox-out/diag/job.log`, lokal saja).
- **Keputusan pemilik:** build dan tes Windows berjalan di **GitHub Actions `windows-latest`** sekarang. Uji interaktif
  (WebView2, installer) menunggu **VM Hyper-V** (Windows 11 Enterprise Evaluation, gratis 90 hari) yang disiapkan
  pemilik sendiri. SAC di host tidak diubah.

**Diverifikasi sesi ini:**
- CI GitHub hijau untuk PR `claude/loving-clarke-ayhpkl` (`7bdaa68`) dan merge ke `main` (`d2e746d`), termasuk runner
  `windows-latest` (fmt, clippy, tes, cargo-deny, cek *drift* binding). Ini menggantikan job sandbox `ipc-contract` yang
  diminta §3.
- Layar awal di Chromium (aplikasi Claude Desktop) pada 1366×768 dan 1920×1080: keadaan penuh, kosong, galat, dan
  memuat tampil benar; tidak ada galat di konsol; fokus Tab terlihat (cincin amber). Uji WebView2 sesungguhnya tetap
  menunggu shell Tauri (Sprint B) dan VM.

**P0-05b (FFmpeg statis): ✅ terverifikasi di GitHub Actions** (run 40068909424, commit `ce84f83`, PR #4).
- Build statis LGPL-2.1+, gerbang rilis, dan probe Rust (9 codec, impor hanya DLL sistem) lulus. Tambahan exe 20,66 MB.
- Butuh 10 putaran CI. Pemilik mengizinkan Claude push **hanya** ke branch `wip/p0-05b-ffmpeg-static` selama iterasi
  ini; izin itu berakhir di sini.
- Kegagalan CI kini terbaca tanpa login: workflow menerbitkan akhir log sebagai *annotation*. Pemantau di sisi Claude
  menghormati batas API GitHub tanpa login (60 permintaan/jam) dan memberi tahu bila gagal atau macet.
- **Langkah pemilik:** merge PR #4 ke `main`. Opsional: Actions → *FFmpeg release build* → Run workflow dengan
  `publish: true` untuk menerbitkan artefak sebagai GitHub Release.
- Untuk exporter Fase 1: tangani paket STREAMINFO akhir FLAC dan jebakan `ffmpeg-next` (laporan P0-05b, temuan 8–9).

**Riwayat lokal dirapikan** (atas izin pemilik): commit `c522310` tanpa DCO diganti oleh commit ber-DCO di branch
P0-05b. Cadangan lengkap ada di branch lokal `backup/pre-rewrite-2026-10-24`, yang boleh dihapus setelah semuanya
di-push.

## 1. Hasil sesi 22 Okt 2026 (cloud)

Semua pekerjaan ada di branch **`claude/loving-clarke-ayhpkl`** (14 commit di atas `main`, belum di-*merge*).

| Pekerjaan | Hasil | Rujukan |
|---|---|---|
| Timebase edit | **Grid flick** (1/705.600.000 s) + snap ke frame (keputusan pemilik) | ADR-0004 Amandemen 2 |
| Inti edit (Rust) | `kinora-project::edit`: `apply` atomik untuk 28 operasi, undo/redo eksak, sesi `Editor` (revisi, *dirty*), `journal.jsonl` untuk pemulihan crash; 120 tes lulus di Linux | [spikes/edit-core-report.md](spikes/edit-core-report.md) |
| Kontrak IPC | `IpcError::editRejected`, `list_recent_projects` + `RecentProject`; binding TS dan skema dibangkitkan | `crates/kinora-ipc` |
| Pedoman frontend | Arsitektur UI, aliran data *read-model*, token, aksesibilitas, proses Impeccable, definisi selesai | [07-frontend-guidelines.md](07-frontend-guidelines.md) |
| Fondasi UI | Svelte 5 + Vite 8 + TS strict; klien IPC bertipe, i18n ID/EN, waktu eksak, tema terang/gelap | `apps/desktop/README.md` |
| Desain (Impeccable 4.7.0) | Dunia visual **"cuci cetak"** (pilihan pemilik), layar awal lengkap (rasio, buka proyek, terakhir dibuka, seret-lepas, keadaan kosong/galat/memuat/proyek hilang); dua putaran review akhir; detektor 0 temuan | `apps/desktop/ui/DESIGN.md`, `PRODUCT.md` |

**Keputusan pemilik yang tercatat:**
- Pemulihan crash hanya mengembalikan riwayat undo sejak simpan terakhir (diterima).
- Perilaku transisi saat trim dipertahankan; diputuskan ulang setelah UI timeline jadi.
- Persona utama UI: kreator awam *short-form*; layar minimum 1366×768.
- Hooks Impeccable boleh aktif.

## 2. Belum diverifikasi (jangan dianggap teruji)

- **CI GitHub belum berjalan untuk commit branch ini.** `ci.yml` hanya terpicu oleh *push* ke `main`, *pull
  request*, atau jalankan manual. Semua pemeriksaan sudah lulus di container Linux, tetapi belum di runner GitHub
  (termasuk `windows-latest`).
- **Job sandbox Windows `ipc-contract`** untuk inti edit (generate, fmt, clippy, test, `cargo deny`).
- **Tampilan di WebView2.** Shell Tauri belum ada; layar awal baru dilihat di Chromium (Linux).
- **Review akhir putaran ketiga.** Tiga regresi putaran kedua (bingkai 9:16 sempit, aksi proyek hilang, keadaan
  kosong) sudah diperbaiki di `ba514c6` dan detektor bersih, tetapi reviewer belum menilai ulang.
- Thumbnail proyek terakhir (butuh cache di shell).

## 3. Langkah pertama di Claude Desktop (Windows)

1. **Ambil branch.** Pemilik memilih salah satu:
   - buka *pull request* `claude/loving-clarke-ayhpkl` → `main`, tunggu CI hijau, lalu *merge*; atau
   - `git fetch origin && git switch claude/loving-clarke-ayhpkl` untuk bekerja langsung di atasnya.

   Sesi lokal tidak pernah *push*; pemilik yang *push* (`CLAUDE.md` §3).
2. `corepack pnpm install`.
3. **Verifikasi inti edit di Windows Sandbox:**
   `powershell -ExecutionPolicy Bypass -File scripts/sandbox/invoke.ps1 -Name ipc-contract -Script scripts/sandbox/jobs/ipc-contract.ps1`.
   Harapan: kedua file kontrak `unchanged`, fmt/clippy/test lulus, `cargo deny` bersih. Catat hasilnya di
   edit-core-report §6 dan baris "Inti edit" di docs/04.
4. **Lihat layar awal di Edge** (mesin Chromium yang sama dengan WebView2):
   `corepack pnpm --filter @kinora/ui run dev`, buka `http://localhost:1420` pada 1366×768 dan layar lebar, juga
   dengan `?state=empty`, `?state=error`, `?state=loading`. Periksa render font (ClearType), kontras kedua tema,
   Tab/fokus, Ctrl+N dan Ctrl+O. Ini belum menggantikan uji WebView2 di dalam shell.
5. **Impeccable di Windows:** binary engine tidak di-commit dan tidak bertanda tangan Authenticode, sehingga Smart
   App Control bisa memblokirnya. **Jangan menonaktifkan Smart App Control.** Jalankan `impeccable detect` dan review
   desain dari sesi cloud atau lewat Windows Sandbox (`.claude/README.md`). Hooks ada di `settings.local.json` per
   mesin dan perlu diaktifkan ulang di mesin baru.

## 4. Pekerjaan berikutnya (urutan yang disarankan)

1. **Sprint B — shell Tauri minimal** (`apps/desktop/src-tauri`). Cocok dikerjakan lokal karena butuh MSVC dan
   WebView2:
   - sesi `Editor` di sisi shell; command buat/buka/simpan proyek (`Name.kinora/`), simpan atomik;
   - tulis jurnal sesuai edit-core-report §3.3 (header saat buka/simpan, tambah dan *sync* tiap entri, tulis ulang
     sesuai ADR-0004 Amandemen 2);
   - daftar proyek terakhir yang tersimpan di folder data aplikasi (`list_recent_projects`, cari folder, hapus dari
     daftar);
   - transport Tauri di `apps/desktop/ui/src/lib/ipc/runtime.ts` (menggantikan `MemoryTransport` di build produksi),
     CSP sesuai docs/07 §10, dialog file, path dari seret-lepas;
   - semua intent layar awal di `App.svelte` yang sekarang masih *no-op* disambungkan;
   - uji layar awal di WebView2; kode khusus Windows memakai `#[cfg(windows)]` dengan *fallback* portabel.
2. **Sisa Fase 0:** P0-05b (FFmpeg rilis statis di CI), P0-10/S4 (installer NSIS + MSIX, uji di Sandbox), P0-13
   (gate go/no-go dan backlog Fase 1), P0-14 (pendaftaran SignPath Foundation dan Microsoft Store oleh pemilik).
3. **Fase 1 (editor MVP)** setelah gate: impor dan *media bin*, timeline di canvas, preview, inspector, ekspor
   (docs/03 §7). Setiap permukaan UI baru memakai Impeccable dan mengikuti `DESIGN.md`.
4. **Perapian kecil UI** (bisa ikut sprint mana pun yang menyentuh berkasnya):
   - nilai yang masih ditulis langsung di komponen dijadikan token: lebar huruf judul 87,5%, garis bingkai rasio
     2px, tinggi print 7,5rem, opasitas 40% dan 50%;
   - token yang belum dipakai (`--status-working`, `--status-done`, `--surface-raised`, `--shadow-lift`,
     `--space-7`, `--space-8`): pakai saat dibutuhkan atau hapus;
   - *open items* di `DESIGN.md`: motif amplop cetak, satu momen gerak yang dirancang, rona tema terang.

## 5. Keputusan yang menunggu pemilik

- Perlu review akhir Impeccable putaran ketiga untuk layar awal, atau langsung lanjut?
- Pilihan sprint berikutnya (saran: Sprint B, shell Tauri minimal).
- Perilaku transisi saat trim, setelah UI timeline jadi.
