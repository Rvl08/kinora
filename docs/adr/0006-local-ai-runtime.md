# ADR-0006 — Runtime AI lokal

- **Status:** Accepted (21 Okt 2026), berdasarkan [Laporan Spike S3](../spikes/S3-report.md); §7 diperbarui oleh [S3b](../spikes/S3b-report.md) pada hari yang sama · **Menggantikan sebagian:** baris "AI" di [ADR-0001](0001-technology-stack.md) (sherpa-onnx dihapus)

## Konteks
Kinora menjanjikan fitur AI yang berjalan sepenuhnya di perangkat: caption otomatis, hapus latar, TTS, dan deteksi jeda.
Syaratnya: tanpa prasyarat (tanpa CUDA, Python, atau runtime terpisah), tanpa komponen GPL (ADR-0002), dan tidak ada data yang dikirim ke luar.
Spike S3 menguji ORT + DirectML, whisper.cpp (Vulkan/CPU), Kokoro, Silero VAD, dan pengelola model di Windows Sandbox (mesin bersih).

## Keputusan
1. **ONNX Runtime (`ort`) + DirectML** untuk semua model ONNX: cutout (BiRefNet), VAD (Silero), TTS (Kokoro), dan model berikutnya.
   - Execution provider DirectML dipasang dengan `error_on_failure`: bila GPU gagal, aplikasi beralih ke CPU **secara eksplisit** dan memberi tahu pengguna, bukan diam-diam lambat.
   - **Telemetri ORT dimatikan** (`with_telemetry(false)`). Build ORT dari Microsoft mengirim telemetri secara default.
   - `DirectML.dll` didistribusikan bersama aplikasi (redistributable Microsoft; ketentuannya dicatat di 05 §2).
2. **whisper.cpp (`whisper-rs`)** untuk speech-to-text.
   - Backend **Vulkan** sebagai jalur utama: RTX 5070 Ti, base RTF 0,021, large-v3-turbo RTF 0,04.
   - Fallback CPU dengan baseline x86-64-v3 (AVX2/FMA/F16C).
   - Rilis memakai `GGML_BACKEND_DL` + `GGML_CPU_ALL_VARIANTS`: backend CPU dipilih saat runtime (CPU tanpa AVX2 tetap jalan), dan backend Vulkan menjadi DLL terpisah (±55 MB shader) yang dapat dimuat atau diunduh hanya bila GPU mendukung.
   - `GGML_NATIVE` **dilarang** di build rilis: probe-nya gagal diam-diam dan hasilnya tidak portabel.
3. **Model default caption:** `large-v3-turbo` (q5_0) bila GPU tersedia. WER Indonesia 4,8%; base 38% dan small 19% tidak layak untuk bahasa Indonesia.
   Tanpa GPU: tetap turbo sebagai pekerjaan latar dengan estimasi waktu, ditambah opsi "cepat" (small/base) dengan peringatan akurasi.
4. **Timestamp kata:** DTW whisper.cpp dengan preset model. Awal kata = waktu DTW **token sebelumnya** (`t_dtw` menandai akhir token), lalu dikoreksi bias (±−40 … −80 ms) dan di-snap ke onset energi atau VAD (Fase 1).
5. **TTS:** Kokoro-82M lewat ORT **langsung**, tanpa sherpa-onnx, dengan **G2P milik sendiri**:
   - Inggris: kamus misaki gold/silver (Apache-2.0) + aturan fallback.
   - Indonesia: aturan ortografi + tekanan penultima.
   - **espeak-ng tidak dipakai** (GPL-3.0).
   - Bahasa Indonesia ditandai **"beta"** sampai ada suara berbahasa Indonesia (05 §4 opsi 3).
6. **VAD:** Silero v5 (ORT, CPU, 1 thread; RTF 0,002).
7. **Cutout:** ORT, sebagai pekerjaan latar dengan hasil di-cache per klip (bukan real-time). Model dipilih per perangkat (S3b §5):
   - **Default: BiRefNet-lite, export dinamis dengan `DeformConv` native** (MIT).
     - 1024² identik dengan export standar (IoU 0,9985) pada **2,4 GB VRAM**, bukan 11,1 GB.
     - 768² di CPU dengan arena mati butuh 1,0 GB RAM.
     - Export standar (`GatherND`) hanya dipakai bila VRAM ≥ 12 GB, karena 10× lebih cepat di DirectML.
   - **Mode "cepat" untuk video:** IS-Net (Apache-2.0), 49 ms/frame dan 1,1 GB VRAM. Kualitas tepinya lebih rendah (halo latar).
   - U²-Net-P ditolak (kualitas). RMBG (BRIA) ditolak (non-komersial).
   - Sebelum rilis, export dinamis dibangun sendiri di CI dari bobot resmi (skrip dipin), bukan memakai file pihak ketiga.
   - Masih terbuka (S3b §5): DeformConv kemungkinan tidak dijalankan DirectML (0,9 dtk/frame), dan stabilitas temporal video belum diukur.
8. **Pengelola model:** model tidak dibundel di installer. Model diunduh saat fitur pertama kali dipakai, dari URL HF yang dipin ke commit.
   - SHA-256 wajib; file kecil non-LFS diverifikasi git-blob SHA-1 lalu SHA-256-nya dipin.
   - Unduhan dapat dilanjutkan (HTTP Range) ke `.part`, lalu rename atomik.
   - Unduhan tanpa digest ditolak.
9. **VC++ runtime** (`msvcp140*.dll`, `vcruntime140*.dll`) dibundel **app-local** oleh installer. Windows bersih tidak memilikinya, sedangkan whisper.cpp dan ORT membutuhkannya.

## Konsekuensi
- ➕ Satu runtime (ORT) untuk mayoritas model; whisper.cpp untuk ASR. Keduanya tanpa CUDA atau Python dan jalan di driver GPU standar.
- ➕ Seluruh rantai TTS bebas GPL; dependensi sherpa-onnx dihapus.
- ➖ Ukuran: `DirectML.dll` 18,5 MB (9,3 MB gzip); ORT + whisper.cpp statis ±28 MB (9,5 MB gzip); backend Vulkan +55 MB (+16 MB gzip) → harus dipisah menjadi DLL (keputusan 2).
- ➖ Kualitas TTS Indonesia belum layak produksi (WER bolak-balik 24–47%).
- ➖ Cutout kualitas tinggi yang hemat memori masih lambat di GPU (±0,9 dtk/frame pada 1024²): video panjang butuh mode "cepat" (IS-Net) atau optimasi export (S3b §5a).
- ➖ Pemanasan pipeline Vulkan ±10 dtk saat model pertama kali dipakai → jalankan pemanasan di latar setelah model diunduh.

## Belum diputuskan
- Export BiRefNet yang cepat di DirectML (varian `GridSample` atau DeformConv fp16), lihat S3b §5a.
- Sumber suara TTS Indonesia berlisensi permisif.
- Validasi di iGPU Intel/AMD dan perangkat 8 GB RAM (perangkat belum ada).
