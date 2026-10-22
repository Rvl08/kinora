# ADR-0004 — Format proyek, waktu, dan Command

- **Status:** Accepted (19 Okt 2026), diamandemen 22 Okt 2026 (Amandemen 1, P0-12)

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

