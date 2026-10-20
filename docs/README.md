# Kinora — Dokumen Perencanaan

| # | Dokumen | Isi |
|---|---|---|
| 01 | [Laporan Reverse Engineering](01-reverse-engineering-report.md) | Temuan CapCut 9.5 & Concat 0.2.7: stack, modul, fitur, model paywall, lisensi, ledger bukti |
| 02 | [Matriks Fitur](02-feature-matrix.md) | ±130 fitur: CapCut vs Concat vs Kinora, beserta fase & pendekatan implementasi |
| 03 | [Master Plan](03-master-plan.md) | Visi, pembeda, stack, arsitektur, installer, pengujian, roadmap 6 fase, risiko, keputusan |
| 04 | [Rencana Fase 0](04-phase0-plan.md) | Audit lingkungan dev, backlog P0-01..13, spesifikasi spike S1–S4, jadwal, gate |
| 05 | [Kepatuhan Lisensi](05-license-compliance.md) | Kelas lisensi, penegakan CI, audit komponen/model/aset, isu espeak-ng |
| S1 | [Laporan Spike S1](spikes/S1-report.md) | Pipeline preview: hasil terukur, temuan, syarat produksi |
| S2 | [Laporan Spike S2](spikes/S2-report.md) | Decode/encode/ekspor: kecepatan, kualitas (VMAF), zero-copy, OpenH264 Cisco, komponen FFmpeg rilis |

## Architecture Decision Records
| ADR | Judul | Status |
|---|---|---|
| [0001](adr/0001-technology-stack.md) | Stack teknologi (Tauri 2 + Svelte 5 + Rust) | Accepted |
| [0002](adr/0002-license-and-clean-room.md) | Lisensi MIT OR Apache-2.0 & clean-room | Accepted |
| [0003](adr/0003-preview-pipeline.md) | Pipeline preview WebCodecs + WebGPU | Accepted untuk GPU diskrit (iGPU menunggu) |
| [0004](adr/0004-project-format-and-commands.md) | Format proyek, waktu rasional, Command | Accepted |
| [0005](adr/0005-codec-and-ffmpeg-policy.md) | FFmpeg 8.1 LGPL & kebijakan codec | Accepted (Amandemen 1–2) |

Status: **Fase 0 berjalan. Selesai: P0-02, P0-03, P0-05, Spike S1 (GPU diskrit), Spike S2 (NVIDIA). P0-04 (CI) menunggu remote GitHub. Berikutnya: S3 (AI), S4 (installer), P0-05b (build FFmpeg rilis), P0-06 (code signing); validasi iGPU (S1) dan QSV/AMF (S2) butuh perangkat lain. P0-01 (nama) ditunda.**
