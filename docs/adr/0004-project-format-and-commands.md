# ADR-0004 — Format proyek, waktu, dan Command

- **Status:** Accepted (19 Okt 2026)

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
