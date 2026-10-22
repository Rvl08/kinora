# Laporan Inti Edit — `apply`, undo/redo, jurnal (pra-Fase 1)

> **Status:** Selesai di sesi cloud (22 Okt 2026), branch `claude/loving-clarke-ayhpkl`. Diuji di Linux; **belum** dijalankan di Windows Sandbox (§6).
> **ADR terkait:** [ADR-0004](../adr/0004-project-format-and-commands.md) Amandemen 2 · [Laporan P0-12](P0-12-report.md)
> **Kode:** [`crates/kinora-core/src/time.rs`](../../crates/kinora-core/src/time.rs) · [`crates/kinora-project/src/edit/`](../../crates/kinora-project/src/edit) · [`crates/kinora-ipc/src/error.rs`](../../crates/kinora-ipc/src/error.rs)

## 1. Ringkasan

| Deliverable | Hasil | Lokasi |
|---|---|---|
| Timebase edit | Grid flick global (1/705.600.000 s), keputusan pemilik | `kinora-core` `time.rs` |
| `apply` | Atomik, deterministik, tervalidasi; 28 operasi | `edit/mod.rs`, `edit/clip_ops.rs` |
| Undo/redo | Patch privat yang memulihkan dokumen identik bit per bit | `edit/patch.rs`, `edit/tx.rs` |
| Sesi `Editor` | Revisi, undo/redo, batas riwayat, status *dirty* berbasis ID state | `edit/history.rs` |
| Jurnal | `journal.jsonl` format 1, pemulihan crash | `edit/journal.rs` |
| Galat | `EditRule` (18 kode terlokalisasi) → `IpcError::editRejected{rule, reason, command}` | `edit/error.rs`, `kinora-ipc` |
| Tes | 120 tes lulus (core 38, project 69, ipc 13), termasuk property test sesi acak dan 4 tes regresi hasil review | `tests/` per crate |

## 2. Timebase edit: grid flick

Masalah dari P0-12 §9 butir 0: menjumlahkan rasional dengan penyebut koprima membuat penyebut melewati `u32`, sehingga edit gagal tanpa alasan yang terlihat oleh pengguna.

| Opsi | Kelebihan | Kekurangan | Keputusan |
|---|---|---|---|
| **Grid flick global** | Penjumlahan tidak pernah membengkak; media dengan rate berbeda tetap eksak; audio sub-frame bisa | Hasil kali kecepatan harus dibulatkan (≤ 0,7 ns) | ✅ dipilih pemilik |
| Snap ke frame timeline | Sederhana | Timebase media tetap tercampur (`sourceIn`); tidak ada edit audio sub-frame | ✖ |

**Rincian:**
- `FLICKS_PER_SECOND = 705.600.000 = 2^9 · 3^2 · 5^5 · 7^2`.
- API: `Time::from_flicks`, `flicks`, `is_on_grid`, `round_to_grid`, `scale_to_grid`/`div_to_grid` dengan `Rounding::{Nearest, Down}`, serta `FrameRate::is_grid_aligned`.
- Rentang ±(2^53 − 1) flick ≈ ±147 hari, sehingga waktu grid dikonversi tepat ke bilangan bulat flick.
- Timebase MP4 umum 1/15360 **tidak** ada di grid. Ini tidak masalah: metadata media (durasi, frame rate stream) bebas. Yang harus di grid hanya waktu sumbu timeline, termasuk `sourceIn` hasil edit (dibulatkan).

## 3. Desain inti edit

### 3.1 Alur `apply`

```
apply(project, command)
├─ check_size        kedalaman batch ≤ 8, ≤ 100.000 command
├─ execute (Tx)      tiap command → aturan sendiri (EditRule) → op primitif, langsung diterapkan
│                    (batch: command berikutnya melihat hasil command sebelumnya)
├─ validate()        jaring pengaman atas seluruh dokumen
└─ commit → Patch    atau rollback (inverse op, urutan terbalik) bila ada galat
```

**Op primitif** (`patch.rs`, privat):
- sisip/hapus untuk media, track, klip, transisi, dan marker;
- ganti klip, ganti transisi, ganti header track;
- `ShiftClips` (ripple).

Setiap op mengembalikan inverse-nya. Patch berisi op berurutan; menerapkan patch menghasilkan patch kebalikannya, sehingga undo dan redo adalah operasi yang sama.

**Alasan memakai patch, bukan inverse berupa `EditCommand`:** banyak command membuang data tambahan yang tidak bisa dinyatakan ulang sebagai command. Contohnya fade yang dipangkas saat trim, transisi yang hilang karena klip dipindah, dan ID efek hasil split. Patch menyimpan nilai lama apa adanya, sehingga undo selalu eksak. Memorinya kecil karena hanya entitas yang berubah yang disimpan, bukan seluruh track atau dokumen.

### 3.2 Semantik command

| Command | Perilaku |
|---|---|
| `splitClip` | Belahan kiri mempertahankan ID. Belahan kanan memakai `newClip`, dan ID efeknya = UUIDv5(belahan kanan, efek asal). Keyframe dan kata caption ada di kedua belahan (relatif terhadap konten). Fade-out pindah ke belahan kanan, fade-in tetap di kiri. Transisi keluar dipindah ke belahan kanan. |
| `trimClip` | Titik sumber di bawah tepi yang tidak digeser tetap (lihat tabel di `clip_ops.rs`, termasuk `reverse`). Keyframe dan kata ikut bergeser bila tepi awal digeser. Fade dipangkas agar muat. Transisi di tepi yang digeser dihapus. |
| `setSpeed` | `start`, `sourceIn`, dan panjang sumber tetap. Durasi timeline = panjang sumber ÷ kecepatan, **dibulatkan ke bawah**, agar rentang sumber tidak pernah melewati media. |
| `moveClip` | Hanya ke track sejenis di timeline yang sama. Transisi klip itu dihapus. Tidak boleh tumpang tindih. |
| `removeClip` | Transisi klip ikut terhapus. Dengan `ripple`, klip berikutnya di track itu mundur sebesar durasinya. |
| `insertClip` | Waktu harus di grid, durasi > 0, ID baru, dan harus ada ruang. Isinya diperiksa validator. |
| `addTransition` | Klip harus bersebelahan (`notAdjacent`). Satu transisi per potongan (`transitionExists`). Transisi tidak boleh lebih panjang dari klipnya (`transitionTooLong`). |
| `removeMedia` | Ditolak bila media masih dipakai klip atau menjadi sumber aset hasil AI (`inUse`). |
| Setter properti | Track terkunci dicek; properti harus ada pada klip (`notApplicable`). Nilai yang sama dengan nilai sekarang menghasilkan patch kosong, tanpa langkah undo dan tanpa entri jurnal. |
| `batch` | Atomik. Galat menyebut command yang gagal (`command: "/params/commands/2"`). |

### 3.3 Sesi dan jurnal

- **Konkurensi.** `Editor::apply`/`undo`/`redo` membawa `baseRevision`; permintaan basi menghasilkan `revisionConflict`.
- **Status *dirty*.** Setiap state dokumen punya ID; setiap langkah menyimpan `before → after`, dan *dirty* = state kini ≠ state tersimpan. Model ini tetap benar setelah undo, redo, riwayat lama dibuang, maupun replay jurnal.
- **Jurnal.** Format lengkap ada di ADR-0004 Amandemen 2 butir 8.
  - Kasus sulit yang ditangani: **undo melewati simpan**. Contoh: simpan, undo, 50 edit, lalu crash. Replay biasa akan berhenti di undo itu (langkahnya tidak ada di jurnal) dan 50 edit hilang. Karena itu entri undo/redo untuk langkah yang lebih tua dari jurnal membawa patch-nya.
  - Saat replay, stack milik replay dipakai lebih dulu. Patch dari entri hanya dipakai bila stack itu kosong, karena stack live = bagian pra-jurnal + bagian replay.
- **Tanggung jawab shell (Fase 1):**
  1. Saat membuka proyek: `from_json` → `journal::read` → `Editor::recover`, atau `Editor::new` bila tidak ada jurnal. Setelah `recover`, **ganti** file jurnal dengan `Replay::journal` sebelum menambah entri apa pun.
  2. `begin_journal(fingerprint(bytes))`, lalu tulis header.
  3. Setiap entri dari `Outcome.journal`: *append* + *sync*.
  4. Simpan: tulis `project.json` secara atomik, lalu `mark_saved`, `begin_journal`, dan ganti jurnal.

## 4. Pengujian

| Tes | Isi |
|---|---|
| `kinora-core` unit (19) + properti (19) | Rate umum *grid-aligned*; konversi flick; pembulatan setengah flick (positif dan negatif); `scale`/`div` (termasuk `Down`); penjumlahan grid tetap di grid; pembulatan ≤ ½ flick; monoton; `Down` tidak pernah memperbesar rentang sumber; frame *aligned* selalu di grid |
| `kinora-project` unit (37) | `EditRule`/pointer batch; jurnal: vektor uji FNV-1a, *round-trip*, file lain/basi/format baru ditolak, ekor terpotong, baris rusak, revisi melompat; `Editor`: undo/redo/revisi, konflik, redo dihapus edit baru, no-op, penolakan, *dirty* melewati undo/redo/pemangkasan, **regresi pemulihan *dirty*** |
| `kinora-project/tests/edit.rs` (17) | Setiap command pada proyek contoh. Tiap kasus sukses diperiksa: dokumen tersimpan dan terbaca ulang identik, undo memulihkan dokumen asli persis, dan redo mengulang edit persis. Penolakan diperiksa kode `EditRule`-nya dan dokumen tidak berubah. |
| `kinora-project/tests/edit_props.rs` | Sesi acak (1–40 aksi: edit, batch, undo, redo, **simpan di tengah**). Diperiksa: tolakan tidak mengubah apa pun; tanpa entri jurnal berarti tidak ada perubahan; hasil tersimpan dan terbaca ulang; pemulihan dari simpanan terakhir + jurnal = dokumen, revisi, dan *dirty* yang sama (label undo juga sama bila tidak ada simpan); undo semua = dokumen asli; redo sejumlah langkah yang di-undo = dokumen akhir. |
| `kinora-project/tests/document.rs` | Kasus semantik baru: keyframe tidak urut, keyframe dan awal klip di luar grid, frame rate tidak *aligned* |
| `kinora-ipc` unit | `editRejected{rule, reason, command}`, `notFound`, `nothingToUndo`, `internal` |

**Distribusi hasil** (1.000 sesi acak, ±19.000 aksi): ±9.100 edit diterima, ±580 no-op. Penolakan tersebar di semua aturan (`trackLocked`, `overlap`, `outsideClip`, `emptyClip`, `notApplicable`, `trackKind`, `otherTimeline`, `sourceRange`, …). Tidak ada `internal`. Jaring pengaman validator hanya tertangkap 13 kali, semuanya transisi yang menjadi terlalu panjang; kasus ini sekarang punya aturan sendiri (`transitionTooLong`).

**Bug yang ditemukan property test dan sudah diperbaiki:** setelah urutan *edit → simpan → undo → crash*, sesi hasil pemulihan melaporkan dokumen bersih padahal berbeda dari file. Akibatnya menutup aplikasi tidak akan meminta simpan. Penyebabnya, *dirty* dilacak lewat "langkah teratas stack undo", yang tidak bisa merepresentasikan state di bawah dasar stack. Diganti dengan ID state eksplisit; tes regresi: `recovery_after_an_undo_across_a_save_is_dirty`.

**Review adversarial oleh sub-agen** (baca-saja; setiap temuan diverifikasi ulang dengan tes regresi yang gagal sebelum perbaikan; semua di `tests/edit_regressions.rs`):

| # | Temuan | Perbaikan |
|---|---|---|
| 1 (tinggi) | Setelah pemulihan yang berhenti di baris terpotong, entri baru yang ditambahkan ke file lama tertahan di belakang baris itu dan hilang pada crash berikutnya | `Replay::journal` berisi jurnal bersih yang wajib ditulis shell sebelum menambah entri; dokumentasi format diperbarui |
| 2 (sedang) | `recover` selalu memakai batas riwayat bawaan, sehingga sesi dengan batas lebih besar gagal di-*replay* (`nothingToUndo`) | Header jurnal menyimpan `historyLimit` |
| 3 (sedang) | `setSpeed` ke kecepatan yang sama memanjangkan klip beberapa flick (efek pembulatan), sehingga muncul langkah undo palsu atau `overlap` | Kecepatan yang sama = no-op |
| 4 (rendah) | Doc `timeline_duration` tidak akurat; durasi transisi ≤ 0 dan marker negatif hanya tertangkap sebagai `invalidResult` | Doc diperbaiki; aturan `outOfRange` yang tepat |

Area yang dinyatakan aman oleh reviewer:
- matematika sumber trim/split/speed (maju dan *reverse*);
- `require_room`, indeks op setelah pergeseran, dan *slicing*;
- aturan track terkunci di semua command;
- atomisitas dan undo eksak;
- invarian stack replay;
- `time.rs` dan validator.

**Tes property yang tidak stabil (diperbaiki):** `rounding_to_the_grid_is_within_half_a_flick` sesekali gagal karena generator menghasilkan nilai di luar rentang grid. Itu kesalahan tes, bukan kode. Seluruh workspace kemudian dijalankan dengan 3.000 kasus per property test: 120 lulus, 0 gagal.

**Pemeriksaan lain (Linux):**
- `cargo fmt --check`;
- `clippy -D warnings` dengan dan tanpa `--all-features`;
- `cargo deny check` (licenses, bans, sources, advisories);
- `pnpm run lint`, `format:check`, `licenses:check`;
- `tsc --strict` atas `bindings.ts`.

Semua lulus. Dependensi baru hanya fitur `v5` dari `uuid`, yang menarik `sha1_smol` 1.0.1 (BSD-3-Clause).

## 5. Performa (release, Linux, mesin cloud)

| Ukuran proyek | `validate` | Satu edit (apply + validasi) | Undo |
|---|---|---|---|
| 1.000 klip | 1,6 ms | 1,3 ms | — |
| 10.000 klip (`project.json` 17 MB) | 13,6 ms | 13,2 ms | ±10 µs |

- Validasi penuh mendominasi biaya.
- Optimasi yang sudah diterapkan: pelacakan ID duplikat tidak lagi mengalokasikan path per entitas (sebelumnya 17 ms untuk 10.000 klip).
- Untuk edit diskret, angka ini jauh di bawah persepsi. Drag interaktif sebaiknya dipratinjau di UI dan di-*commit* saat dilepas (§7).

## 6. Belum diverifikasi

- **Windows.** Crate ini portabel dan CI menjalankannya di `windows-latest`, tetapi belum dijalankan di mesin pemilik. Langkah verifikasi di host Windows: `powershell -ExecutionPolicy Bypass -File scripts/sandbox/invoke.ps1 -Name ipc-contract -Script scripts/sandbox/jobs/ipc-contract.ps1`. Job ini menjalankan `generate` (kedua file harus `unchanged`), fmt, clippy, test, dan `cargo deny`.
- **Shell Tauri** (I/O jurnal, *sync* ke disk, simpan atomik) belum ada. Bagian ini masuk Fase 1.

## 7. Pertanyaan terbuka dan tindak lanjut

1. **Performa validasi untuk proyek sangat besar:** buat path JSON Pointer di validator menjadi *lazy*, atau validasi hanya track yang tersentuh. Target: < 2 ms untuk 10.000 klip.
2. **Transisi saat trim:** saat ini transisi di tepi yang digeser dihapus, dan transisi yang menjadi terlalu panjang ditolak. Alternatif yang perlu diputuskan saat UI dibuat: *roll edit* (kedua klip ikut bergeser) atau memendekkan transisi otomatis.
3. **Split caption:** teks diduplikasi ke kedua belahan (kata ditandai per waktu). Memecah teks per kata bergantung bahasa; perlu diputuskan saat fitur transkrip (Fase 3).
4. **Ripple multi-track** (*sync lock*): `ripple` saat ini hanya untuk satu track.
5. **Batas riwayat berdasarkan memori**, bukan hanya jumlah langkah (bawaan 500).
6. **Fuzzing** `from_json`, `journal::read`, dan `edit::apply` dengan `cargo-fuzz` (butuh nightly; bisa dijalankan di sesi cloud).
7. Pemulihan tidak membangun ulang riwayat undo **sebelum** simpan terakhir. Ini perilaku umum aplikasi desktop; perlu dikonfirmasi pemilik.
8. **Bolak-balik kecepatan** (r1 → r2 → r1) bisa menggeser akhir klip beberapa flick karena pembulatan, walaupun sumber yang diputar tetap sama. Usulan: planner men-snap durasi hasil `setSpeed` ke frame timeline.
