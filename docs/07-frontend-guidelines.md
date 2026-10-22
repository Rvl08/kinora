# 07 — Pedoman Arsitektur & Desain Frontend (UI)

> **Status:** Disetujui pemilik untuk dipakai sejak Fase 1 (22 Okt 2026). Mengikat semua kode di `apps/desktop/ui`.
> **Dasar:** [03 §6.6](03-master-plan.md) (prinsip UX) · [ADR-0001](adr/0001-technology-stack.md) (stack) · [ADR-0003](adr/0003-preview-pipeline.md) (preview) · [ADR-0004](adr/0004-project-format-and-commands.md) (command, revisi, jurnal) · [Laporan P0-12](spikes/P0-12-report.md) §5 (kontrak IPC) · `CLAUDE.md` §4.
> Perubahan pedoman ini dilakukan lewat PR yang menyebut alasannya. Keputusan besar (mis. mengganti framework state) memerlukan ADR.

## 1. Tujuan dan ukuran keberhasilan

Kinora harus terasa **tenang, cepat, dan jelas**. Pengguna awam bisa membuat video 9:16 dengan teks dan musik dalam < 10 menit (exit Fase 1). Editor pro bisa bekerja sepenuhnya dari keyboard.

| Ukuran | Target | Cara mengukur |
|---|---|---|
| Cold start → layar awal | ≤ 2 dtk (03 §6.4) | Tauri `performance.now()` sejak `DOMContentLoaded` + timer native |
| Bundle JS awal (gzip) | ≤ 150 KB (layar awal); fitur berat dimuat malas | `vite build` + laporan ukuran di CI |
| Respons interaksi (klik, tombol) | ≤ 100 ms sampai umpan balik terlihat | Profil DevTools WebView2 |
| Timeline & preview | 60 fps saat scrub/drag pada proyek 1.000 klip | Profil frame |
| Aksi umum | ≤ 2 klik (03 §6.6) | Tinjauan desain per fitur |
| Aksesibilitas | WCAG 2.2 AA | Impeccable `detect` + uji keyboard + pembaca layar (Narrator) |

## 2. Stack dan alasannya

| Pilihan | Keputusan | Alasan |
|---|---|---|
| Framework | **Svelte 5** (runes), **tanpa SvelteKit** | Aplikasi satu jendela tanpa SSR dan routing URL. Vite + plugin Svelte lebih kecil dan lebih sederhana. |
| Bahasa | **TypeScript `strict`** + `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes` | Tipe kontrak IPC dibangkitkan dari Rust; aturan ketat menangkap kesalahan kontrak saat kompilasi. `any` dilarang (ESLint). |
| Build | **Vite** | Standar ekosistem Svelte; HMR cepat; keluaran statis untuk Tauri. |
| Tes | **Vitest** (logika, store, i18n); `@testing-library/svelte` untuk komponen bila dibutuhkan; `tauri-driver` untuk E2E (Windows) | Satu runner dengan konfigurasi Vite yang sama. |
| Cek tipe Svelte | **svelte-check** | Memeriksa `.svelte` dan `.svelte.ts` dengan `tsconfig` yang sama. |
| Desain | **Impeccable** wajib (`CLAUDE.md` §4), dijalankan dari baris UI pertama | Konteks produk (`/impeccable init`) dan deteksi *anti-pattern* (`impeccable detect`). |
| i18n | **Modul sendiri** (≈150 baris, bertipe, tanpa dependensi) | Kunci pesan dicek saat kompilasi; tidak ada runtime pihak ketiga; cukup untuk ID/EN + `Intl`. Dievaluasi ulang bila bahasa > 4. |
| Ikon | Set ikon open-license yang dipilih bersama Impeccable (mis. Lucide, ISC), diimpor per ikon | Ukuran kecil; tidak ada *icon font*. |

**Aturan dependensi:**
- Setiap paket baru harus lolos `pnpm run licenses:check`, dipin ke versi **pasti**, dan dijelaskan di deskripsi PR: untuk apa, ukurannya (gzip), dan alternatif yang ditolak.
- Dependensi runtime UI harus sedikit. Utamakan platform web (`Intl`, `<dialog>`, Popover API, CSS modern) yang tersedia di WebView2 evergreen.

## 3. Struktur folder

```
apps/desktop/ui/
├─ index.html                 entri; CSP diatur oleh Tauri (§10)
├─ package.json               paket workspace `@kinora/ui`
├─ vite.config.ts             alias `$lib`, konfigurasi Vitest
├─ svelte.config.js
├─ tsconfig.json
└─ src/
   ├─ main.ts                 mount App, pasang tema & bahasa
   ├─ App.svelte              kerangka: layar awal ↔ ruang kerja editor
   ├─ app.css                 reset + impor token
   └─ lib/
      ├─ ipc/                 bindings.ts (DIBANGKITKAN), transport.ts, client.ts, errors.ts
      ├─ state/               store read-model: project, jobs, preferences (*.svelte.ts)
      ├─ i18n/                index.svelte.ts, messages/{id,en}.ts, format.ts
      ├─ time/                konversi Rational ↔ tampilan (timecode, detik) di tepi UI
      ├─ theme/               tokens.css, theme.svelte.ts (terang/gelap/sistem)
      ├─ components/          primitif UI: Button, IconButton, Dialog, Menu, Tooltip, Toast, …
      ├─ features/            start/, timeline/, preview/, inspector/, media/, transcript/, copilot/
      └─ keyboard/            registri pintasan + daftar pintasan yang bisa diubah
```

**Aturan:**
- **`features/<nama>/`** berisi komponen dan logika satu fitur. Fitur tidak mengimpor isi fitur lain; yang dipakai bersama naik ke `lib/components`, `lib/state`, atau modul `lib/*` lain.
- **`lib/components/`** hanya berisi primitif tanpa pengetahuan domain (tidak mengenal `Clip` atau `Project`).
- Logika non-visual ditulis di `.ts` atau `.svelte.ts` (bila memakai runes) agar bisa diuji tanpa DOM. Komponen `.svelte` tetap tipis.
- `bindings.ts` **tidak pernah diedit tangan** (dibangkitkan oleh `pnpm run ipc:generate`, diuji *drift*).

## 4. Aliran data: UI sebagai *read-model*

Sumber kebenaran adalah dokumen di Rust (`Editor`, ADR-0004). UI **tidak pernah** mengubah dokumen secara lokal.

```
aksi pengguna ──► EditCommand ──invoke("apply_command", {baseRevision, command})──► Rust Editor
       ▲                                                                              │
       │                         event "project:changed" {state}                     │
       └── store.project ◄── get_project (bila revisi tertinggal) ◄──────────────────┘
```

1. **Edit = command.** Setiap perubahan dikirim sebagai `EditCommand` (`apply_command`) bersama `baseRevision` yang sedang ditampilkan UI. Undo/redo memakai `undo`/`redo` dengan `baseRevision`.
2. **Tarik, bukan dorong.** Event `project:changed` hanya membawa `ProjectState`. Bila revisinya lebih baru, store memanggil `get_project`. Satu permintaan per revisi; permintaan basi dibuang.
3. **`revisionConflict`** berarti UI tertinggal. Store menyegarkan diri dan tidak mengulang edit secara otomatis; pengguna melihat hasil terbaru.
4. **Interaksi berkelanjutan** (drag klip, trim, scrub) memakai **state sementara lokal** untuk pratinjau visual, lalu satu command saat dilepas. Jangan mengirim command per frame drag.
5. **Data besar tidak lewat IPC JSON** (ADR-0003): paket video, waveform, dan thumbnail memakai respons biner atau `cache/`.

**Svelte 5 runes:**
- Dokumen proyek disimpan dengan **`$state.raw`** (tanpa *deep proxy*); setiap revisi mengganti seluruh objek. Ini wajib untuk performa proyek besar.
- `$derived` untuk turunan murah (daftar track, durasi timeline). Turunan mahal dihitung di Rust atau di-*memo* per revisi.
- `$effect` hanya untuk sinkronisasi ke luar Svelte (canvas, event listener), selalu dengan *cleanup*. Jangan memakai `$effect` untuk menurunkan state.
- Store adalah kelas di `*.svelte.ts` dengan field `$state` privat dan getter publik. Komponen tidak mengubah field store secara langsung.
- Nilai yang berubah 60×/detik (posisi playhead saat playback) **tidak** disimpan di `$state`. Canvas membacanya di `requestAnimationFrame` dari sumber jam (ADR-0003).

## 5. Lapisan IPC

| Modul | Isi |
|---|---|
| `ipc/transport.ts` | Antarmuka `Transport { invoke, listen }`. Implementasi Tauri (`@tauri-apps/api`, ditambahkan bersama shell Tauri) dan `MemoryTransport` untuk tes. |
| `ipc/client.ts` | `call(name, args)` bertipe dari peta `Commands`; `on(event, handler)` dari peta `Events`. Galat dinormalisasi menjadi `IpcError`. |
| `ipc/errors.ts` | `IpcError` → kunci pesan i18n. `switch` eksaustif (pemeriksaan `never`) sehingga kode galat baru dari Rust gagal dikompilasi sampai diberi pesan. |

**Aturan:**
- Komponen tidak memanggil `invoke` langsung; selalu lewat store atau `client`.
- Setiap panggilan bisa gagal; UI selalu menampilkan galat dalam bahasa pengguna, bukan teks mentah dari Rust (`reason` hanya untuk log/dev).
- `editRejected` memakai `rule` untuk memilih penjelasan; `internal` menampilkan ID insiden yang bisa disalin.

## 6. Internasionalisasi (ID/EN sejak hari pertama)

- **Bahasa Indonesia adalah bahasa sumber.** Bahasa Inggris harus memenuhi bentuk yang sama; tipe TypeScript memaksa kedua kamus punya kunci identik.
- Kunci bersifat semantik dan bertingkat: `start.newProject`, `errors.editRule.overlap`. Teks UI tidak pernah ditulis langsung di komponen.
- Interpolasi `{nama}`; bentuk jamak dengan `Intl.PluralRules`; angka, tanggal, dan durasi dengan `Intl` sesuai bahasa aktif.
- Bahasa awal dari `navigator.language` (`id*` → Indonesia, selain itu Inggris), dapat diubah dan disimpan di preferensi. Atribut `lang` pada `<html>` selalu sesuai.
- **Gaya bahasa:** Indonesia yang natural dan ringkas, bukan terjemahan kaku. Contoh: "Proyek baru", bukan "Buat sebuah proyek baru". Kata kerja di tombol, istilah teknis yang lazim dipakai kreator (klip, timeline, ekspor) tidak dipaksa diterjemahkan. Kalimat galat menjelaskan apa yang terjadi dan apa yang bisa dilakukan.
- **Clean-room:** teks UI ditulis sendiri; tidak menyalin teks dari CapCut atau Concat (`CLAUDE.md` §3).

## 7. Sistem desain

Nilai akhir (palet, tipografi, skala) ditetapkan bersama **Impeccable** (`/impeccable init`, lalu iterasi dengan `impeccable detect`). Struktur berikut mengikat sejak awal.

### 7.1 Token
- Semua nilai visual lewat **CSS custom properties** di `theme/tokens.css`, dalam tiga lapis:
  1. **primitif**: warna dalam **OKLCH** agar kecerahan dan kontras terkendali, dinamai menurut dunia visual (sekarang `--pigment-*`, lihat `apps/desktop/ui/DESIGN.md`) dan hanya dirujuk di `tokens.css`;
  2. **semantik** (`--surface-base`, `--surface-sunken`, `--text-primary`, `--text-secondary`, `--border-subtle`, `--accent`, `--status-danger`, `--focus-ring`, …);
  3. **komponen** (hanya bila perlu).
- Komponen hanya memakai token **semantik**. Warna heksa langsung di komponen ditolak saat review.
- **Tema terang dan gelap** didefinisikan ulang di lapis semantik (`[data-theme="dark"]`); pilihan bawaan mengikuti sistem (`prefers-color-scheme`) dan bisa diubah. Editor video lazim bekerja di tema gelap agar warna footage tidak terganggu, sehingga tema gelap dirancang sebagai warga kelas satu, bukan hasil inversi.
- Skala spasi berbasis 4 px; radius, bayangan, dan durasi gerak juga berupa token.

### 7.2 Tipografi
- Huruf badan teks dan kontrol: **font sistem Windows** (Segoe UI Variable) dengan *fallback* sistem. Font ini nol biaya, tajam di ClearType, dan tidak menambah ukuran installer.
- Satu **font display** berlisensi terbuka boleh dipakai untuk judul dan label bergaya (sekarang Archivo, OFL, *variable*, subset Latin ±90 KB). Syaratnya: di-*bundle* (tidak diunduh saat runtime), lisensi dan SHA-256 dicatat di sebelah berkasnya, dan pemakaiannya didokumentasikan di `DESIGN.md`.
- Font berlisensi terbuka untuk *teks video* adalah hal terpisah (aset proyek).
- Angka timecode dan durasi memakai `font-variant-numeric: tabular-nums` agar tidak "menari".
- Skala tipe kecil dan konsisten (mis. 12/13/15/18/24); badan teks UI minimal 13 px.

### 7.3 Prinsip visual (anti "AI slop")
- **Konten adalah bintang.** Preview dan timeline mendapat ruang dan kontras terbesar; kerangka UI netral dan tenang.
- **Satu warna aksen**, dipakai hemat untuk aksi utama, seleksi, dan fokus. Tidak ada gradien ungu generik, teks bergradien, atau aksen bercahaya.
- **Hierarki lewat ruang dan tipografi**, bukan kotak di dalam kotak. Hindari kartu seragam yang berderet dan border di setiap elemen.
- **Ikon hanya bila bermakna** dan selalu memiliki label (teks atau `aria-label` + tooltip). Tidak ada ikon dekoratif.
- **Gerak** singkat (120–200 ms), *easing* standar (tanpa *bounce*), dan dimatikan saat `prefers-reduced-motion`. Perpindahan dan perubahan ukuran hanya lewat `transform`/`opacity` (tidak menganimasikan properti tata letak). Transisi warna atau border pada perubahan status (hover, fokus, aktif) boleh, karena tidak memicu tata letak.
- Kepadatan informasi mengikuti persona: mode sederhana di depan, kontrol pro satu klik di belakang (progressive disclosure, 03 §6.6).

## 8. Pola UX wajib

| Pola | Aturan |
|---|---|
| Progressive disclosure | Panel inspector menampilkan kontrol umum lebih dulu; kontrol lanjutan di bagian yang bisa dibuka, dan statusnya diingat. |
| ≤ 2 klik | Impor, split, hapus, teks, ekspor, undo dapat dicapai ≤ 2 klik dan punya pintasan. |
| Job AI dan ekspor | Selalu terlihat, jujur (progres nyata + ETA bila diketahui), dan **selalu bisa dibatalkan**. Tidak ada spinner tanpa keterangan. |
| Galat | Menjelaskan apa yang terjadi dan langkah berikutnya, dalam bahasa pengguna. Edit yang ditolak tidak mengubah apa pun, dan UI mengatakannya. |
| Undo | Setiap edit bisa di-undo; label undo/redo memakai nama aksi yang diterjemahkan. |
| Keadaan kosong | Layar kosong menjelaskan langkah pertama (mis. "Seret video ke sini"), bukan area kosong tanpa petunjuk. |
| Konfirmasi | Hanya untuk aksi yang tidak bisa di-undo (menutup tanpa simpan, menghapus file). Aksi yang bisa di-undo tidak perlu dialog. |

## 9. Aksesibilitas dan keyboard

- Kontras WCAG 2.2 AA: teks ≥ 4,5:1, komponen UI dan fokus ≥ 3:1, di **kedua** tema.
- Semua aksi dapat dicapai dengan keyboard. Fokus selalu terlihat (`:focus-visible` dengan token `--focus-ring`); urutan fokus mengikuti urutan visual.
- Elemen HTML semantik lebih dulu (`<button>`, `<dialog>`, `<menu>`), ARIA hanya bila perlu. Komponen kustom (timeline canvas, slider) menyediakan peran, label, dan nilai untuk pembaca layar.
- Status yang berubah (progres job, hasil edit yang ditolak) diumumkan lewat region `aria-live="polite"`.
- Informasi tidak boleh hanya disampaikan lewat warna (status track terkunci memakai ikon + label).
- Target klik minimal 24×24 px (WCAG 2.5.8).
- **Pintasan:** satu registri pusat (`keyboard/`) yang bisa diubah pengguna, tanpa bentrok dengan pintasan sistem. Daftar pintasan tersedia lewat `?`.

## 10. Keamanan

- **CSP ketat** di konfigurasi Tauri: `default-src 'self'`, tanpa `unsafe-eval`, tanpa sumber jarak jauh. Aset media dimuat lewat protokol aset Tauri yang dibatasi ke folder proyek dan cache.
- Tidak ada `{@html}` untuk teks yang berasal dari pengguna, file proyek, transkrip, atau Copilot.
- Path file hanya berasal dari dialog native, drag-drop, atau daftar proyek terbaru. UI tidak menyusun path sendiri; Rust memvalidasi ulang (P0-12 §5.6).
- Tidak ada telemetri, *analytics*, atau permintaan jaringan dari UI. Unduhan model lewat Rust (terverifikasi SHA-256).
- Preferensi disimpan lewat Rust (bukan `localStorage` untuk data penting); `localStorage` hanya untuk hal kosmetik yang boleh hilang.

## 11. Performa

- **Pemuatan malas:** fitur berat (timeline canvas, preview WebGPU, transkrip, Copilot) diimpor dinamis saat dibutuhkan. Layar awal hanya memuat kerangka, i18n, dan tema.
- **Timeline digambar di canvas** dengan virtualisasi (03 §4), bukan ribuan elemen DOM. Daftar panjang (media bin, transkrip) divirtualisasi.
- Tidak ada pembaruan reaktif per frame. Animasi playback dan scrub berjalan di `requestAnimationFrame` yang membaca jam audio.
- `$state.raw` untuk dokumen; hindari menyalin dokumen penuh di komponen.
- Gambar dan thumbnail memakai ukuran yang sesuai tampilan (tidak memuat frame 4K untuk thumbnail).
- Anggaran bundle dicek di CI setelah build UI tersedia.

## 12. Gaya kode dan komponen

- **Komponen:** satu per file, `PascalCase.svelte`, idealnya < 150 baris.
  - Props dideklarasikan dengan `let { … }: Props = $props()` dan `interface Props`.
  - Event berupa *callback prop* (`onclick`, `onchange`), konten berupa *snippet*. Tidak memakai `createEventDispatcher` atau slot lama.
- **Penamaan:** `camelCase` untuk variabel dan fungsi, `PascalCase` untuk tipe dan komponen, `kebab-case` untuk nama file non-komponen, `UPPER_SNAKE` untuk konstanta modul.
- **Gaya CSS:** `<style>` di komponen (*scoped*) untuk tata letak komponen itu; token dari `tokens.css`. Tidak ada framework CSS utilitas (Tailwind, dsb.).
- **Waktu:** `Rational` dari Rust dikonversi ke detik atau timecode hanya di `lib/time` dan hanya untuk tampilan. Nilai yang dikirim ke Rust selalu `Rational` pada grid flick, di-*snap* ke frame timeline sebelum dikirim (ADR-0004 Amandemen 2).
- **Komentar dan kode** dalam bahasa Inggris (dokumen di `docs/` dalam bahasa Indonesia).
- ESLint + Prettier wajib lulus; `any` dan `@ts-ignore` dilarang (gunakan `@ts-expect-error` dengan alasan, hanya di tes).

## 13. Pengujian

| Lapisan | Alat | Wajib untuk |
|---|---|---|
| Logika murni (`lib/time`, `lib/i18n`, `lib/ipc/errors`) | Vitest | Setiap fungsi publik, termasuk kasus batas |
| Store (`*.svelte.ts`) | Vitest + `MemoryTransport` | Alur revisi: tarik dokumen, konflik, galat |
| Komponen | `@testing-library/svelte` (bila perlu) | Komponen dengan perilaku (keyboard, fokus, status) |
| Kontrak tipe | `svelte-check` / `tsc` + fixture `satisfies Project` | Setiap perubahan binding |
| Desain | `impeccable detect` | Setiap PR UI; temuan "primer" harus nol atau dijelaskan |
| E2E | `tauri-driver` (Windows Sandbox) | Alur inti per rilis: buat proyek, impor, edit, ekspor |

Perintah dari root repo: `pnpm run ui:check` (svelte-check + tsc untuk konfigurasi), `pnpm run ui:test` (Vitest), `pnpm run ui:build`. Ketiganya juga dijalankan job `web` di CI.

## 14. Proses kerja dengan Impeccable

1. `npx impeccable install --scope=project` (versi dipin; hooks hanya dengan persetujuan pemilik).
2. `/impeccable init`: konteks produk dari 03 §1 (persona), §6.6, dan pedoman ini.
3. Setiap fitur UI:
   - rancang dengan skill Impeccable;
   - implementasikan dengan token semantik;
   - jalankan `impeccable detect` pada folder fitur;
   - perbaiki temuan;
   - periksa kontras dan keyboard di kedua tema.
4. Tangkapan layar terang + gelap disertakan di PR UI.

## 15. Definisi selesai untuk pekerjaan UI

- [ ] Teks dari kamus i18n (ID dan EN lengkap); tidak ada string literal di komponen.
- [ ] Hanya token semantik; terang dan gelap diperiksa; kontras AA.
- [ ] Bisa dioperasikan penuh dengan keyboard; fokus terlihat; label untuk pembaca layar.
- [ ] Galat ditangani dan ditampilkan dalam bahasa pengguna; job bisa dibatalkan.
- [ ] Tes Vitest untuk logika baru; `check`, `lint`, `format:check`, `licenses:check` lulus.
- [ ] `impeccable detect` bersih atau temuannya dijelaskan.
- [ ] Tidak ada dependensi baru tanpa justifikasi (lisensi, ukuran, alternatif).
- [ ] Verifikasi di WebView2 (Windows) dicatat; bila belum dilakukan, disebutkan dengan langkah ujinya.
