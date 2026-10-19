# Kinora — Dokumen Perencanaan

| # | Dokumen | Isi |
|---|---|---|
| 01 | [Laporan Reverse Engineering](01-reverse-engineering-report.md) | Temuan CapCut 9.5 & Concat 0.2.7: stack, modul, fitur, model paywall, lisensi, ledger bukti |
| 02 | [Matriks Fitur](02-feature-matrix.md) | ±130 fitur: CapCut vs Concat vs Kinora, beserta fase & pendekatan implementasi |
| 03 | [Master Plan](03-master-plan.md) | Visi, pembeda, stack, arsitektur, installer, pengujian, roadmap 6 fase, risiko, keputusan |
| 04 | [Rencana Fase 0](04-phase0-plan.md) | Audit lingkungan dev, backlog P0-01..13, spesifikasi spike S1–S4, jadwal, gate |
| 05 | [Kepatuhan Lisensi](05-license-compliance.md) | Kelas lisensi, penegakan CI, audit komponen/model/aset, isu espeak-ng |

## Architecture Decision Records
| ADR | Judul | Status |
|---|---|---|
| [0001](adr/0001-technology-stack.md) | Stack teknologi (Tauri 2 + Svelte 5 + Rust) | Accepted |
| [0002](adr/0002-license-and-clean-room.md) | Lisensi MIT OR Apache-2.0 & clean-room | Accepted |
| [0003](adr/0003-preview-pipeline.md) | Pipeline preview WebCodecs + WebGPU | Proposed (Spike S1) |
| [0004](adr/0004-project-format-and-commands.md) | Format proyek, waktu rasional, Command | Accepted |
| [0005](adr/0005-codec-and-ffmpeg-policy.md) | FFmpeg 8.1 LGPL & kebijakan codec | Accepted (Amandemen 1) |

Status: **Fase 0 berjalan. P0-02, P0-03, dan P0-05 (FFmpeg 8.1 LGPL untuk dev) selesai; P0-04 (CI) selesai secara lokal dan menunggu remote GitHub. Berikutnya P0-06 (code signing) dan spike S1–S4. P0-05b (build FFmpeg rilis) menunggu S2. P0-01 (nama) ditunda.**
