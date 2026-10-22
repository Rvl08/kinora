# ADR-0004 — Format proyek, waktu, dan Command

- **Status:** Accepted (19 Okt 2026), diamandemen 22 Okt 2026 (Amandemen 1, P0-12; Amandemen 2, inti edit)

## Konteks
Edit harus frame-accurate, dapat di-undo tanpa batas, dapat dipulihkan setelah crash, dan dapat dijalankan oleh UI,
AI Copilot, server MCP, maupun skrip — semuanya melalui jalur yang sama.

## Keputusan
1. **Waktu = bilangan rasional** `{num: i64, den: u32}` dalam detik. Tidak ada `f64` di model dokumen. Konversi ke float hanya di tepi (UI/render).
2. **Proyek = folder** `NamaProyek.kinora/`:
   - `project.json` — dokumen lengkap, ber-`schema_version` (semver), di-*migrate* maju secara otomatis.
   - `journal.jsonl` — command yang belum di-*checkpoint* (append-only, fsync berkala) → pemulihan crash.
   - `snapshots/` — riwayat versi (fitur P4).
   - `cache/` — proxy, thumbnail, waveform, transkrip (boleh dihapus; dapat dibangun ulang).
   - Media **direferensikan** (path absolut + relatif + hash cepat xxh3 dari 3 potongan) untuk relink; opsi "Kumpulkan media" menyalin ke `media/`.
3. **Semua mutasi = `Command`** (enum serializable, `op` + `params`). Setiap command: `validate → apply → inverse`.
   `Batch` atomik untuk operasi majemuk (mis. hapus filler = puluhan split + ripple delete = **satu** langkah undo).
4. **Transkrip terikat ke media**, bukan ke klip → tetap valid setelah klip dipotong/digeser.
5. **Skema JSON publik** (dibangkitkan dari tipe Rust via `schemars`) — dipakai validator, API, dan generator tipe TypeScript.
6. Paket portabel: `.kinorapkg` (zip folder proyek + media terpilih) untuk berbagi/backup.

## Konsekuensi
- ➕ Satu jalur edit untuk UI/Copilot/MCP → perilaku dan penolakan konsisten; Copilot cukup menghasilkan daftar command.
- ➕ JSON mudah di-diff, di-backup, dan diinspeksi pengguna.
- ➖ Migrasi skema wajib diuji di setiap rilis (fixture proyek per versi di `tests/fixtures/projects/`).

## Amandemen 1 — Kontrak tipe & format v0 (22 Okt 2026, P0-12)
Rincian dan alasan: [Laporan P0-12](../spikes/P0-12-report.md).
1. **Waktu** tetap `{num: i64, den: u32}`, dengan tambahan:
   - `|num| ≤ 2^53 − 1` (aman untuk JavaScript);
   - selalu dalam bentuk terkecil;
   - aritmetika *checked*, tanpa pembulatan diam-diam;
   - diimplementasikan di `kinora-core`.

   **Belum diputuskan:** timebase edit (snap ke frame timeline atau tick global *flick* 1/705.600.000 s), agar penyebut tidak tumbuh saat penjumlahan. Wajib diputuskan sebelum `apply` diimplementasikan di Fase 1.
2. **`schemaVersion`** (camelCase, seperti seluruh JSON) adalah **integer** yang naik pada setiap perubahan format, bukan semver.
   - Build lama menolak dokumen yang lebih baru.
   - Dokumen lama dimigrasikan maju per langkah pada JSON tak bertipe.
   - Versi 0 adalah draf dan dibekukan menjadi 1 pada rilis pertama yang menulis proyek pengguna.
3. **Parsing ketat:** field dan varian tak dikenal ditolak. Kompatibilitas maju hanya lewat gerbang versi.
4. **ID** entitas = UUIDv7, dibuat pemanggil dan dibawa dalam command (replay jurnal deterministik).
5. **Command** dokumen bernama `EditCommand` di kode, dengan bentuk kabel `{op, params}`.
   - Setiap edit membawa `baseRevision`; edit dengan revisi basi ditolak.
   - Event (`project:changed`, `job:progress`; Tauri 2 melarang `.` di nama event) hanya membawa status, lalu UI menarik dokumen.
6. **Pembangkitan otomatis:**
   - Skema publik dibangkitkan `schemars` ke `crates/kinora-project/schema/project.v<N>.json`.
   - Tipe TypeScript dibangkitkan `ts-rs` (bukan dari skema) ke `apps/desktop/ui/src/lib/ipc/bindings.ts`, bersama kontrak IPC dari crate `kinora-ipc` (bebas dari Tauri, agar dapat dipakai ulang oleh `kinora-api`/MCP).
   - Keduanya diuji *drift* di CI.
7. **Fixture** per versi berada di `crates/kinora-project/tests/fixtures/v<N>/`, menggantikan `tests/fixtures/projects/`.

## Amandemen 2 — Timebase edit, inti edit, dan jurnal (22 Okt 2026)
Rincian, pengujian, dan angka: [Laporan inti edit](../spikes/edit-core-report.md). Menutup pertanyaan terbuka Amandemen 1 butir 1.

1. **Timebase edit = grid flick global** (keputusan pemilik, 22 Okt 2026). Snap ke frame timeline saja ditolak karena tetap mencampur timebase media dan menutup kemungkinan edit audio sub-frame.
   - 1 flick = 1/705.600.000 detik (2^9 · 3^2 · 5^5 · 7^2). Satu flick habis membagi durasi frame 24/25/30/48/50/60/90/100/120 fps, varian NTSC-nya (×1000/1001), periode sampel 8–192 kHz, dan clock MPEG 90 kHz.
   - **Invarian dokumen:** setiap waktu pada sumbu timeline adalah kelipatan bulat flick dalam ±(2^53 − 1) flick (±147 hari). Ini berlaku untuk `start`/`duration` klip, `sourceIn`, `at` keyframe, fade, durasi transisi, marker, dan waktu kata caption. Frame rate timeline wajib *grid-aligned* (satu frame = bilangan bulat flick). Metadata media (`info.duration`, frame rate stream) boleh memakai timebase apa pun.
   - Penjumlahan dan pengurangan waktu grid tetap di grid, sehingga penyebut tidak pernah membengkak.
   - Hasil yang keluar dari grid (durasi × kecepatan) dibulatkan **secara eksplisit** ke flick terdekat (`Time::scale_to_grid`, galat ≤ 0,7 ns). Jika hasilnya tidak boleh melampaui batas, pembulatan dilakukan ke bawah (`Rounding::Down`).
   - **Snap ke frame adalah kebijakan UI/Copilot**, bukan invarian. UI men-snap posisi ke frame timeline sebelum mengirim command. Klip audio boleh berada di posisi sub-frame.
2. **Pemetaan sumber tunggal:** `Speed::source_duration` (dibulatkan ke flick) adalah satu-satunya pemetaan timeline → sumber, dipakai validator, inti edit, dan renderer. Saat tepi klip berpindah, titik sumber di bawah tepi **yang lain** tetap, termasuk untuk klip `reverse`. Akibatnya klip yang berakhir tepat di ujung media tetap berakhir di sana setelah trim, split, atau pembulatan kecepatan.
3. **Aturan validator yang berubah:**
   - Keyframe dan kata caption **boleh berada di luar rentang klip**. Trim dan split mempertahankannya relatif terhadap konten, sehingga animasi tetap menyambung dan muncul kembali saat klip diperpanjang.
   - Satu potongan (*cut*) memiliki paling banyak satu transisi.
4. **`inverse` berupa patch privat, bukan `EditCommand`.** Setiap command dijalankan sebagai urutan operasi primitif (sisip/hapus/ganti entitas, geser klip). Inverse setiap operasi menyimpan data yang dihapus atau diganti, sehingga undo memulihkan dokumen **identik bit per bit**, termasuk data yang sengaja dibuang command (fade yang dipangkas, transisi yang hilang karena klip dipindah). Undo dan redo adalah operasi yang sama (menerapkan patch menghasilkan patch kebalikannya).
5. **Jaminan `apply`:** atomik (termasuk `batch`), deterministik (ID baru dibawa command atau diturunkan darinya, misalnya ID efek pada belahan kanan split = UUIDv5), dan tervalidasi. Aturan per command menghasilkan kode `EditRule` yang dapat dilokalkan. Setelah itu validator penuh dijalankan sebagai jaring pengaman; hasil yang melanggar format di-*rollback*. Input dibatasi: kedalaman batch ≤ 8, ≤ 100.000 command per edit.
6. **Aturan edit Fase 1:**
   - Track terkunci tidak bisa diubah isinya (`updateTrack` tetap boleh agar bisa dibuka kuncinya).
   - Klip pada satu track tidak boleh tumpang tindih. Command tidak menggeser klip lain, kecuali `removeClip` dengan `ripple`.
   - Transisi milik potongannya: command yang menggeser salah satu tepi di potongan itu menghapus transisinya (undo mengembalikannya). Transisi tidak boleh lebih panjang dari klipnya (`transitionTooLong`).
   - Kosakata ditambah 14 operasi properti: `updateTrack`, `renameClip`, `setClipEnabled`, `setTransform`, `setCrop`, `setOpacity`, `setBlendMode`, `setAudioMix`, `setText`, `setCaption`, `addTransition`, `removeTransition`, `addMarker`, `removeMarker`.
7. **Sesi (`Editor`):** revisi dengan konkurensi optimistik, undo/redo dengan batas riwayat (bawaan 500 langkah), dan status *dirty* berbasis ID state (setiap state dokumen punya ID; *dirty* = state kini ≠ state tersimpan). Setiap operasi mengembalikan entri jurnal yang harus ditulis shell, sehingga shell tidak bisa mencatat hal yang salah.
8. **`journal.jsonl` format 1** (menjawab pertanyaan terbuka P0-12 §9 butir 3):
   - Baris pertama berisi *header* `{kind:"header", format, schemaVersion, project, base, revision, historyLimit}`. `historyLimit` adalah batas undo penulis; replay memakai batas yang sama agar tidak melupakan langkah yang berbeda dari sesi aslinya. `base` adalah sidik FNV-1a 64-bit dari byte `project.json`. Jurnal yang tidak cocok dengan file di disk (misalnya crash di antara menulis `project.json` dan memotong jurnal) dianggap basi dan tidak di-*replay*.
   - Entri berikutnya: `apply{revision, command}`, `undo{revision, step}`, `redo{revision, step}`. Undo/redo atas langkah yang lebih tua dari jurnal (dibuat sebelum simpan terakhir) membawa patch langkah itu di `step`, karena replay tidak bisa membangunnya ulang.
   - Pembaca menoleransi baris terakhir yang terpotong dan berhenti di baris rusak pertama. Ukuran dibatasi (jurnal 256 MiB, baris 16 MiB). Replay memvalidasi semua yang diterapkannya.
   - Pemulihan membangun ulang dokumen, revisi, status *dirty*, dan riwayat undo **sejak simpan terakhir**.
   - Pemulihan bisa berhenti sebelum akhir file (baris terpotong atau rusak, atau entri yang gagal). Karena itu shell **mengganti** jurnal dengan `Replay::journal` (header + entri yang berhasil di-*replay*) **sebelum** menambah entri baru. Tanpa langkah ini, entri baru akan tertahan di belakang baris rusak dan hilang pada pemulihan berikutnya.
   - Shell (Fase 1) menulis header saat membuka proyek dan setelah setiap simpan. Setiap entri ditambahkan lalu di-*sync* ke disk. Simpan: tulis `project.json` secara atomik (file sementara lalu *rename*), lalu mulai jurnal baru.

