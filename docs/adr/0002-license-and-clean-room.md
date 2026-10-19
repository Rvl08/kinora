# ADR-0002 — Lisensi proyek & kebijakan clean-room

- **Status:** Accepted (19 Okt 2026) · **Keputusan terkait:** D1 (opsi a)

## Konteks
Referensi kita: CapCut (proprietary, ByteDance) dan Concat (AGPL-3.0-or-later). Pengguna memilih membangun baru
dengan lisensi permisif agar bebas dipakai, didistribusikan, dan dikembangkan tanpa kewajiban copyleft.

## Keputusan
1. **Lisensi kode Kinora: `MIT OR Apache-2.0`** (dual, konvensi ekosistem Rust). Apache-2.0 memberi *patent grant* eksplisit; MIT memberi kesederhanaan.
2. **Clean-room ketat:**
   - ❌ Dilarang menyalin kode, shader WGSL, preset, manifest efek, teks UI, atau dokumentasi dari Concat.
   - ❌ Dilarang memakai aset/biner/model/string/merek/ikon CapCut, dan dilarang mendekompilasi DLL CapCut.
   - ✅ Boleh: daftar fitur, perilaku yang dapat diamati, dan pola arsitektur umum (mis. *command pattern*, waktu rasional) — ide tidak dilindungi hak cipta.
   - ✅ Laporan RE (`docs/01`, `docs/02`) menjadi satu-satunya jembatan pengetahuan; developer yang mengimplementasikan efek tidak membuka source Concat selama implementasi.
3. **Setiap dependensi & model** wajib lolos kebijakan di [05-license-compliance.md](../05-license-compliance.md) sebelum di-merge.
4. **CLA tidak dipakai;** kontribusi memakai **DCO** (`Signed-off-by`) di bawah lisensi yang sama.

## Konsekuensi
- ➕ Bebas untuk distribusi gratis maupun penggunaan komersial pihak ketiga; kompatibel dengan hampir semua dependensi permisif & LGPL (dinamis).
- ➖ **Tidak boleh** menautkan komponen GPL/AGPL (x264, x265, espeak-ng, RVM, Concat). Butuh alternatif — lihat 05 §4.
- ➖ Seluruh katalog efek (≥120 paket) harus ditulis sendiri.
