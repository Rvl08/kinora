# Kinora — Dokumen Perencanaan

| # | Dokumen | Isi |
|---|---|---|
| 01 | Catatan analisis CapCut & Concat | Internal pemelihara, **tidak dipublikasikan** (keputusan 21 Okt 2026, [06](06-public-release-plan.md) §4). Ringkasan fiturnya ada di 02 |
| 02 | [Matriks Fitur](02-feature-matrix.md) | ±130 fitur: CapCut vs Concat vs Kinora, beserta fase & pendekatan implementasi |
| 03 | [Master Plan](03-master-plan.md) | Visi, pembeda, stack, arsitektur, installer, pengujian, roadmap 6 fase, risiko, keputusan |
| 04 | [Rencana Fase 0](04-phase0-plan.md) | Audit lingkungan dev, backlog P0-01..13, spesifikasi spike S1–S4, jadwal, gate |
| 05 | [Kepatuhan Lisensi](05-license-compliance.md) | Kelas lisensi, penegakan CI, audit komponen/model/aset, isu espeak-ng |
| 06 | [Rencana Publikasi & Distribusi Tanpa Biaya](06-public-release-plan.md) | Microsoft Store (MSIX) + SignPath Foundation tanpa biaya, dampak arsitektur, audit kesiapan repo publik, keputusan yang dibutuhkan |
| S1 | [Laporan Spike S1](spikes/S1-report.md) | Pipeline preview: hasil terukur, temuan, syarat produksi |
| S2 | [Laporan Spike S2](spikes/S2-report.md) | Decode/encode/ekspor: kecepatan, kualitas (VMAF), zero-copy, OpenH264 Cisco, komponen FFmpeg rilis |
| S3 | [Laporan Spike S3](spikes/S3-report.md) | AI lokal: Whisper Vulkan/CPU + timestamp kata, BiRefNet DirectML, Kokoro tanpa espeak-ng, Silero VAD, memori, pengelola model |
| S3b | [Laporan Spike S3b](spikes/S3b-report.md) | Model cutout hemat memori: BiRefNet export dinamis vs IS-Net vs U²-Net-P, memori, kualitas tepi |
| P0-11 | [Laporan fixture media uji](spikes/P0-11-report.md) | 15 fixture berlisensi bebas: sumber, lisensi, verifikasi, celah |

## Architecture Decision Records
| ADR | Judul | Status |
|---|---|---|
| [0001](adr/0001-technology-stack.md) | Stack teknologi (Tauri 2 + Svelte 5 + Rust) | Accepted |
| [0002](adr/0002-license-and-clean-room.md) | Lisensi MIT OR Apache-2.0 & clean-room | Accepted |
| [0003](adr/0003-preview-pipeline.md) | Pipeline preview WebCodecs + WebGPU | Accepted untuk GPU diskrit (iGPU menunggu) |
| [0004](adr/0004-project-format-and-commands.md) | Format proyek, waktu rasional, Command | Accepted |
| [0005](adr/0005-codec-and-ffmpeg-policy.md) | FFmpeg 8.1 LGPL & kebijakan codec | Accepted (Amandemen 1–2) |
| [0006](adr/0006-local-ai-runtime.md) | Runtime AI lokal (ORT + DirectML, whisper.cpp + Vulkan, TTS tanpa espeak-ng) | Accepted (§7 cutout diperbarui S3b) |
| [0007](adr/0007-code-signing.md) | Code signing (semua exe/dll, pipeline tidak terikat penyedia; Certum Open Source) | Accepted |

Status: **Fase 0 berjalan. Selesai: P0-02, P0-03, P0-05, Spike S1 (GPU diskrit), Spike S2 (NVIDIA), Spike S3 (AI, GPU diskrit), Spike S3b (cutout). P0-04 (CI hijau di GitHub, repo publik `Rvl08/kinora`). P0-06: pipeline signing siap dan teruji; distribusi tanpa biaya (Microsoft Store + SignPath Foundation, [06](06-public-release-plan.md)). Berikutnya: S4 (installer), P0-05b (build FFmpeg rilis); validasi iGPU (S1, S3) dan QSV/AMF (S2) butuh perangkat lain. P0-01 (nama) ditunda.**
