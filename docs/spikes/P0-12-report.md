# Laporan P0-12 — Kontrak IPC & Skema `project.json` v0

> **Status:** Selesai (22 Okt 2026), belum di-commit (menunggu review lead).
> **ADR terkait:** [ADR-0004](../adr/0004-project-format-and-commands.md) (diusulkan Amandemen 1, §8.4) · [ADR-0003](../adr/0003-preview-pipeline.md) · [ADR-0001](../adr/0001-technology-stack.md)
> **Kode:** [`crates/kinora-core`](../../crates/kinora-core) · [`crates/kinora-project`](../../crates/kinora-project) · [`crates/kinora-ipc`](../../crates/kinora-ipc)
> **Rekomendasi:** ✅ **`ts-rs` 12.0.1** untuk tipe Rust → TypeScript, **`schemars` 1.2.3** untuk JSON Schema, dan **manifest command/event milik sendiri** di `kinora-ipc` (tidak bergantung Tauri). `specta`/`tauri-specta` ditolak untuk saat ini (§2).

## 1. Ringkasan

| Deliverable | Hasil | Lokasi |
|---|---|---|
| Pilihan generator tipe | `ts-rs` 12.0.1 (dipin `=`), `schemars` 1.2.3 (dipin `=`) | `Cargo.toml` (workspace) |
| Waktu rasional | `Rational`/`Time`/`FrameRate`, aritmetika *checked*, aman untuk JS | `crates/kinora-core` |
| Model dokumen v0 | `Project` → media pool, timeline, track, klip, efek, keyframe, transisi, teks, caption, marker | `crates/kinora-project/src` |
| Versi & migrasi | `schemaVersion` integer, mesin migrasi bertahap + tes | `crates/kinora-project/src/version.rs` |
| Validasi semantik | 40+ aturan, laporan berupa JSON Pointer | `crates/kinora-project/src/validate.rs` |
| JSON Schema v0 | dibangkitkan, di-*check in*, diuji *drift* | `crates/kinora-project/schema/project.v0.json` |
| Fixture contoh | 4 media, 2 timeline (16:9 + 9:16 NTSC), 5 jenis track | `crates/kinora-project/tests/fixtures/v0/sample.kinora/project.json` |
| Kontrak IPC | 9 command, 2 event, `IpcError` | `crates/kinora-ipc/src` |
| Binding TS | dibangkitkan, header `@generated`, diuji *drift* | `apps/desktop/ui/src/lib/ipc/bindings.ts` |

Bukti uji: §6.

## 2. Keputusan: generator tipe

### 2.1 Kandidat (versi per 21 Okt 2026)

| Kriteria | `specta` 2.0.0-rc.25 + `tauri-specta` 2.0.0-rc.25 | **`ts-rs` 12.0.1** |
|---|---|---|
| Typing command/event Tauri 2 | ✅ bawaan (`collect_commands!`, `#[derive(Event)]`), tetapi **mengikat kontrak ke Tauri** dan butuh crate `tauri` di generator | ❌ tidak ada → diganti manifest sendiri (§5), yang juga dipakai ulang oleh `kinora-api` (JSON-RPC/MCP) |
| Status rilis & perawatan | **RC sejak 2023**; rc.23 (Mar 2026) merombak API (serde dipisah ke `specta-serde`), rc.26 terbit **hari ini** → churn tinggi, satu maintainer utama | **Stabil (semver)**, rilis teratur (10.x 2024 → 12.0 Jan 2026), ±19 juta unduhan |
| Lisensi | MIT (lolos `deny.toml`) | MIT (lolos `deny.toml`) |
| Kompatibilitas serde | Ketat: mode *unified* menolak `skip_serializing_if`; `deny_unknown_fields` diabaikan | `rename_all`, `rename_all_fields`, `tag`, `content`, `untagged`, `flatten`, `skip_serializing_if` (→ field opsional) didukung |
| `f64` | Selalu `number \| null` (hardcoded; NaN → `null`) → seluruh model jadi *nullable*; override butuh `specta_typescript::Number` di crate model | `number` |
| Integer 64-bit | Default **gagal** (bagus) | Default `bigint` (keliru untuk JSON) → dipasang **sentinel** di `Config::with_large_int` sehingga generasi **gagal** kecuali field diberi `#[ts(type = "number")]` dan dibatasi 2^53 − 1 → perilaku setara specta |
| Generik | ✅ | ✅ (`Animated<T>`, `Keyframe<T>`) |
| Cerita CI | Ekspor ke string → bandingkan | `decl()` per tipe + kunjungan dependensi → satu file deterministik → bandingkan (§7) |

Alternatif lain yang ditimbang:
- **JSON Schema → TS** (`json-schema-to-typescript`): butuh tool Node di jalur build Rust, union `oneOf` jadi kurang presisi, dan tidak menghasilkan peta command/event. Skema tetap dipakai sebagai kontrak validasi publik (ADR-0004 §5), bukan sumber TS.
- **`typeshare`**: dukungan enum *internally tagged* dengan data terbatas; ditolak.

### 2.2 Alasan
1. **Stabilitas kontrak.** Binding di-*check in* dan dibandingkan byte-per-byte; generator yang API-nya berubah tiap RC membuat setiap upgrade menjadi pekerjaan migrasi. `ts-rs` stabil.
2. **Kebenaran tipe.** `number | null` untuk setiap `f64` (specta) adalah bohong bagi model kita: dokumen menolak NaN (lihat §4.6), sehingga nilai `null` tidak pernah dikirim.
3. **Kontrak tidak bergantung Tauri.** ADR-0004 menuntut satu jalur edit untuk UI, Copilot, MCP, dan skrip. Manifest command di `kinora-ipc` bisa dipakai `kinora-api` apa adanya; `tauri-specta` hanya mengetik `invoke` Tauri.
4. Biaya: wrapper `invoke`/`listen` bertipe (±10 baris) ditulis sekali di UI (contoh di §5.5) — sebanding dengan yang dihasilkan `tauri-specta`.

**Dipin:** `ts-rs = "=12.0.1"` (fitur `serde-compat`, `no-serde-warnings`, `uuid-impl`), `schemars = "=1.2.3"` (fitur `uuid1`). Dependensi lain memakai caret + `Cargo.lock`: `serde` 1.0.229, `serde_json` 1.0.154 (fitur `float_roundtrip`), `uuid` 1.28.0 (`v7`, `serde`), `thiserror` 2.0.21; khusus tes: `proptest` 1.12.0, `jsonschema` 0.59.0 (tanpa fitur default → tanpa HTTP/TLS).

Generator menambahkan dua pengaman di atas `ts-rs`:
- Menolak `*/` di dalam doc comment Rust. `ts-rs` menyalin doc ke JSDoc apa adanya, sehingga `effects/*/effect.toml` di sebuah doc menutup komentar lebih awal dan merusak file. Ini ditemukan saat verifikasi dengan `tsc`.
- Membuang spasi di akhir baris.

`no-serde-warnings` dipakai karena `ts-rs` mencetak peringatan untuk atribut yang tidak memengaruhi bentuk TS (`deny_unknown_fields` pada enum, `try_from`). Bentuk TS dijaga oleh tes (§6), termasuk pemeriksaan `tsc` atas fixture.

## 3. Struktur crate & lokasi binding

| Crate | Isi | Dependensi |
|---|---|---|
| `kinora-core` | `Rational`, `Time`, `FrameRate`, `MAX_SAFE_INTEGER` | **std saja**; `serde`/`schemars`/`ts-rs` opsional (fitur) |
| `kinora-project` | Model `project.json`, ID, `EditCommand` (bentuk saja), validasi, versi/migrasi, skema | `kinora-core`, serde, schemars, uuid, thiserror; `ts-rs` opsional |
| `kinora-ipc` (**baru**) | Trait `IpcCommand`/`IpcEvent`, args/hasil, `IpcError`, generator binding | `kinora-project`, ts-rs (selalu) |

`kinora-ipc` belum ada di daftar crate 03 §5.1. Alasannya: kontrak harus bisa diuji tanpa `tauri` (crate besar dengan WebView2), dan dipakai bersama oleh `src-tauri` (Fase 1) dan `kinora-api`. Usulan edit 03 di §8.3.

**Lokasi binding: `apps/desktop/ui/src/lib/ipc/bindings.ts`.** Sesuai struktur 03 §5.5 (`apps/desktop/ui` = Svelte), dan `src/lib` adalah alias `$lib` standar Svelte/Vite, sehingga UI cukup `import type { … } from '$lib/ipc/bindings'`. File ini hanya berisi deklarasi tipe (tanpa kode runtime, tanpa import) dan diawali header `@generated`.

## 4. Skema `project.json` v0

### 4.1 Bentuk

```
Project { schemaVersion, id, name, settings, media[], timelines[] }
├─ settings: { defaultFormat{width,height,frameRate}, colorSpace, audio{sampleRate,channels} }
├─ media[]: MediaAsset { id, name, kind, location{absolutePath,relativePath}, hash{algorithm,digest,fileSize},
│                        info{duration, video{…}, audio{…}}, origin(imported | generated{generator,source}) }
└─ timelines[]: Timeline { id, name, format, tracks[], markers[] }
   └─ tracks[]: Track { id, kind(video|audio|text|caption), name, muted, hidden, locked, clips[], transitions[] }
      ├─ clips[]: Clip { id, name, start, duration, enabled, content, visual?, audio? }
      │   ├─ content: media{media,sourceIn,speed,reverse,preservePitch} | text{text,style}
      │   │            | caption{text,words[],style,language} | solid{color} | adjustment
      │   ├─ visual: { transform{position,scale,rotation,anchor,flip*}, crop, opacity, blendMode, effects[] }
      │   │   └─ effects[]: { id, effect{id,version}, enabled, params{name → Animated<ParamValue>} }
      │   └─ audio: { gainDb, fadeIn, fadeOut }
      └─ transitions[]: { id, from, to, duration, effect, params }
```

`Animated<T>` = `{kind:"static", value}` | `{kind:"keyframes", keyframes:[{at, value, easing}]}`; `at` relatif terhadap awal klip, sehingga memindahkan klip ikut memindahkan animasinya. `ParamValue` = `{float}|{int}|{bool}|{vec2}|{color}|{choice}`. `EditCommand` = `{op, params}` (ADR-0004 §3), 14 operasi awal termasuk `batch`; *apply/inverse* menyusul di Fase 1.

### 4.2 Keputusan: waktu
- **Semua waktu = `{num, den}` detik** (ADR-0004 §1), tidak ada `f64` untuk posisi/durasi/kecepatan/frame rate.
- **`num` dibatasi ±(2^53 − 1)** agar `JSON.parse` di UI tetap eksak (≈ 285 tahun pada resolusi 1 µs). Tanpa batas ini, `i64` di JSON diam-diam kehilangan presisi di JavaScript.
- **Selalu dalam bentuk terkecil** (`2/4` dibaca sebagai `1/2`; ditulis kanonik) → `Eq`/`Hash` turunan = kesetaraan numerik, dan file yang sama selalu menghasilkan byte yang sama.
- **Aritmetika *checked*** di `i128`: hasil di luar jangkauan (pembilang > 2^53 atau penyebut > `u32::MAX`) → `None`, tidak pernah dibulatkan atau *wrap*.
- **Kecepatan rasional** (`2/1`, `1/2`): durasi sumber = `duration × rate` eksak; rentang sumber **diturunkan, tidak disimpan dua kali** (`sourceIn .. sourceIn + duration × speed`), sehingga tidak mungkin tidak konsisten.
- Frame: `Time::from_frames`, `frame_floor` (waktu tepat di batas = frame berikutnya), `frame_round`, `snap_to_frame`; 1 jam NTSC = 107.892 frame tanpa *drift* (diuji).
- `Speed` adalah enum bertag (`{kind:"constant", rate}`) supaya kurva kecepatan (P2) bisa ditambah sebagai varian baru.

### 4.3 Keputusan: ID
- **UUIDv7** (RFC 9562) per jenis entitas, sebagai newtype (`ClipId`, `TrackId`, …) sehingga tidak bisa tertukar di Rust. Unik tanpa koordinasi dan berurutan waktu (diff & indeks rapi).
- ID **dibuat oleh pemanggil** dan dibawa di dalam `EditCommand` (mis. `splitClip.newClip`) → *replay* jurnal deterministik.
- Saat membaca, versi UUID apa pun diterima. Di TS, ID berupa `string` (belum *branded*; §9).
- ID unik di seluruh dokumen, lintas jenis (divalidasi).

### 4.4 Keputusan: versi & migrasi
- **`schemaVersion` = satu integer**, naik pada **setiap** perubahan format, termasuk yang aditif. Tidak memakai semver (berbeda dari ADR-0004 §2), karena editor menulis ulang file: build lama yang "mengabaikan" field baru akan **menghapusnya diam-diam** saat menyimpan. Jadi build lama harus **menolak** file yang lebih baru (`MigrationError::TooNew` → `IpcError::projectTooNew`).
- File lama dimigrasikan maju **langkah demi langkah pada JSON tak bertipe** (`fn(&mut Map) -> Result`), karena tipe Rust lama sudah tidak ada. Mesin migrasi yang mengisi `schemaVersion`, bukan langkahnya. Registry `MIGRATIONS` harus kontigu (`MIGRATIONS[n].from == n`) dan berakhir di versi kini (diuji).
- Jalur cepat: dokumen versi kini langsung di-*deserialize* dari teks (satu pass, galat menyebut baris/kolom); hanya dokumen lama yang lewat `Value`.
- **Versi 0 = draf** Fase 0/1: boleh berubah tanpa migrasi sampai rilis pertama yang menulis proyek pengguna membekukannya menjadi **versi 1**. Sejak itu setiap perubahan = langkah migrasi + fixture `tests/fixtures/v<N>/`.
- Skema JSON memuat `"schemaVersion": {"const": 0}` → satu file skema per versi (`project.v<N>.json`).

### 4.5 Keputusan: field tak dikenal
**Ketat: `deny_unknown_fields` di semua struct dan enum.** Kompatibilitas maju dijamin oleh gerbang versi (§4.4), bukan oleh toleransi field. Alasannya:
- Toleransi tanpa preservasi = kehilangan data saat simpan ulang (lihat §4.4).
- Preservasi (`#[serde(flatten)] extra`) bertabrakan dengan `deny_unknown_fields`, tag internal, schemars, dan ts-rs, serta mengaburkan typo.
- Ketat menangkap typo dari skrip/Copilot/MCP lebih awal. Pesan galat serde menyebut nama field yang diharapkan.

Tes `schema_and_parser_agree_on_structure` memastikan skema JSON dan parser **sama ketatnya** (12 kasus: field asing, field hilang, versi lebih baru, penyebut 0, pembilang > 2^53, waktu float, nilai enum/tag tak dikenal, field varian lain, frame rate ≤ 0, …).

### 4.6 Keputusan lain
- **JSON camelCase** di seluruh dokumen dan IPC (idiomatis di TS; nama `schemaVersion` sesuai brief). ADR-0004 menulis `schema_version` → diamandemen.
- **Setiap field selalu ditulis** (opsional = `null`, tidak dihilangkan). Output kanonik: indentasi 2 spasi, urutan field = urutan deklarasi, kunci map terurut (`BTreeMap`), newline akhir. Pembaca tetap menerima field opsional yang hilang.
- **Float**: nilai visual (opacity, posisi, warna) tetap `f64`. JSON tidak punya NaN/∞; `serde_json` menulis NaN sebagai `null` sehingga file tak bisa dibuka lagi. Maka `to_json` (1) memvalidasi (rentang & finit), lalu (2) **membaca ulang outputnya** dan menolak jika tidak identik. `float_roundtrip` menjamin setiap `f64` finit kembali bit-per-bit (property test).
- **Koordinat relatif kanvas** (pusat = 0, ±0,5 = tepi) dan ukuran teks relatif tinggi kanvas → timeline bisa diubah resolusi/rasio tanpa menggeser apa pun.
- **Media direferensikan** (ADR-0004 §2): `absolutePath` + `relativePath` (relatif folder `.kinora`, pemisah `/`, divalidasi bukan absolut) + `hash{algorithm:"xxh3Sampled", digest(32 hex), fileSize}`. Algoritma pengambilan sampel pastinya ditetapkan `kinora-media` (P1); mengubahnya = varian baru. Data turunan (proxy, thumbnail, waveform, transkrip) tetap di `cache/`, bukan di dokumen.
- **Artefak AI** = aset media dengan `origin: {kind:"generated", generator:"kinora.ai.cutout@1", source}` → bisa dibangkitkan ulang dari sumbernya.
- **Batas ukuran** 64 MiB untuk `project.json` (melindungi dari file rusak/berbahaya).
- **Warna proyek** `rec709` | `rec2100Pq` | `rec2100Hlg` (Fase 1 hanya Rec. 709; varian HDR sudah ada agar P5+ tidak butuh versi baru).
- **Validasi semantik** (`validate()`, dipanggil saat buka dan sebelum simpan) mengembalikan **semua** pelanggaran dengan JSON Pointer, mis. klip tumpang tindih, referensi media/klip menggantung, rentang sumber melewati akhir media, konten tidak cocok dengan jenis track, keyframe tidak urut/di luar klip, tipe parameter efek campur, transisi tidak bersebelahan, kanvas ganjil/di luar 2–16384, fade melebihi klip, kata caption tidak urut.

## 5. Kontrak IPC

### 5.1 Konvensi
- Setiap command menerima **tepat satu parameter `args`**. UI memanggil `invoke(name, { args })`, dan shell mendeklarasikan `fn name(args: <C as IpcCommand>::Args) -> Result<<C as IpcCommand>::Output, IpcError>`. Command dengan `Args = ()` tidak punya parameter.
- `COMMAND_NAMES`/`EVENT_NAMES` dibangkitkan dari manifest. Tes di `src-tauri` (Fase 1) wajib membandingkannya dengan daftar `generate_handler!`.
- **Nama event memakai `:`** (`project:changed`, `job:progress`) karena Tauri 2 hanya mengizinkan alfanumerik, `-`, `/`, `:`, `_`. Penulisan `project.changed` di 03 §5.1 tidak valid; diuji oleh `event_names_follow_tauri_rules`.
- **Dokumen tidak pernah dikirim lewat event.** `project:changed` hanya membawa `ProjectState` (revisi, dirty, label undo/redo); UI mengambil dokumen dengan `get_project` bila revisinya tertinggal (pola *pull* seperti S1, ADR-0003). Paket media tetap lewat respons biner.
- **Konkurensi optimistik:** `apply_command`, `undo`, `redo` membawa `baseRevision`; jika basi → `revisionConflict` (mencegah edit dihitung dari state lama dan *double undo* karena klik ganda).

### 5.2 Command

| Nama | `args` | `output` |
|---|---|---|
| `create_project` | `{parentDir, name, settings}` | `ProjectOpened` |
| `open_project` | `{path}` | `ProjectOpened {snapshot, migratedFrom, recoveredEdits}` |
| `save_project` | `{saveAs: string \| null}` | `ProjectState` |
| `close_project` | `{discardChanges}` | `null` |
| `get_project` | `null` | `ProjectSnapshot {state, document}` |
| `apply_command` | `{baseRevision, command: EditCommand}` | `ProjectState` |
| `undo` / `redo` | `{baseRevision}` | `ProjectState` |
| `cancel_job` | `{job}` | `null` |

### 5.3 Event

| Nama | Payload |
|---|---|
| `project:changed` | `{reason: opened\|edited\|undone\|redone\|saved\|closed, state: ProjectState \| null}` |
| `job:progress` | `{job, kind: export\|proxy\|transcription\|analysis\|modelDownload, state: queued \| running{progress, etaMs} \| succeeded \| failed{error} \| cancelled}` (maks. 10 Hz per job, status akhir selalu dikirim) |

### 5.4 Desain galat (`IpcError`)
- Diserialisasi `{ "code": "<kode>", …detail }`. UI memilih teks terlokalisasi (ID/EN) berdasarkan `code`; `Display` berbahasa Inggris hanya untuk log.
- Kode: `noProject`, `unsavedChanges`, `notFound{entity,id}`, `invalidArgument{argument,reason}`, `revisionConflict{expected,current}`, `editRejected{reason}`, `nothingToUndo`, `nothingToRedo`, `projectTooNew{found,supported}`, `projectInvalid{problems[≤20]{pointer,message}, omitted}`, `io{kind}`, `cancelled`, `internal{incident}`.
- **Tidak membocorkan internal:**
  - `From<io::Error>` hanya menyimpan **jenis** (`notFound`, `permissionDenied`, `storageFull`, `busy`, …), bukan pesan OS yang bisa memuat path (diuji dengan path `C:\Users\…`).
  - `From<LoadError>` hanya membawa JSON Pointer dan pesan parser/validator Kinora (baris/kolom, bukan path file).
  - Selain itu → `internal{incident}` (UUIDv7). Shell mencatat detail di log lokal dengan ID yang sama.
  - Tidak ada `From<String>`/`anyhow` → galat bebas tidak bisa lolos ke UI tanpa disengaja.

### 5.5 Pemakaian di UI (Fase 1, contoh)

```ts
import { invoke } from '@tauri-apps/api/core';
import type { Commands } from '$lib/ipc/bindings';

export function call<K extends keyof Commands>(cmd: K, args: Commands[K]['args']) {
  return invoke<Commands[K]['output']>(cmd, { args });
}
```

### 5.6 Catatan keamanan untuk shell (Fase 1)
- `open_project`/`create_project`/`save_project` menerima path dari WebView. Shell **wajib** menerima hanya folder `*.kinora` berisi `project.json` (dari dialog native/drag-drop/daftar terbaru), melakukan kanonikalisasi, dan menolak path perangkat (`\\.\`, `\\?\` mentah). Path tidak pernah dipantulkan kembali di galat.
- Sebelum menulis ulang file hasil migrasi, simpan cadangan `project.v<lama>.json`.

## 6. Pengujian & bukti

| Tes | Isi |
|---|---|
| `kinora-core` unit (11) | normalisasi, penolakan penyebut 0/pembilang tak aman, aritmetika eksak, overflow → `None`, floor/ceil/round, urutan, bentuk JSON, NTSC 1 jam, batas frame, kecepatan eksak |
| `kinora-core/tests/rational_props.rs` (13 properti, proptest) | konstruksi kanonik & tak pernah panik (input `i64`/`u32` ekstrem), skala n/d, komutatif/asosiatif, invers +/−, ×/÷, distributif, urutan vs tanda selisih & vs `f64`, floor/ceil/round mengapit nilai, *round-trip* frame, frame yang memuat waktu, *round-trip* JSON |
| `kinora-project` unit | ID v7 & bentuk string, versi/migrasi (registry kontigu, *pass-through*, urutan langkah, versi lebih baru, langkah gagal, celah registry, 7 versi rusak, `SchemaVersion` bertipe), `EditCommand` `{op,params}` & penolakan op/field asing, I/O (bukan JSON, bukan objek, tanpa versi, terlalu besar, *round-trip* deterministik, tolak NaN), helper validasi |
| `kinora-project/tests/document.rs` | skema mutakhir (*drift*), fixture lolos skema (`jsonschema`), fixture dimuat + **kanonik** (tulis ulang = nilai JSON sama), 12 kasus struktur (skema ≡ parser), 12 kasus semantik (pointer tepat), normalisasi rasional |
| `kinora-project/tests/props.rs` (proptest) | setiap `f64` finit bit-eksak, keyframe, `EditCommand`, dokumen dengan waktu acak *save → load* identik |
| `kinora-ipc` unit | bentuk `IpcError`, **tidak bocor path**, pemetaan `LoadError`, `internal` hanya incident, kunci map, **penolakan integer 64-bit tanpa anotasi**, binding ter-render |
| `kinora-ipc/tests/contract.rs` | **binding mutakhir** (generate di memori vs file), nama command = fungsi Rust, nama event sesuai aturan Tauri, nama unik |
| `tsc --strict` (manual, §7) | fixture `satisfies Project` (cek properti berlebih di seluruh pohon), contoh `IpcError`/`EditCommand`/event, wrapper `call<K>` bertipe, 3 kasus `@ts-expect-error` |

**Bukti:**
- **Windows Sandbox**, job `ipc-contract` (`scripts/sandbox/jobs/ipc-contract.ps1`), log di `tests/sandbox-out/ipc-contract/job.log` (tidak di-commit). Langkah: `generate`, lalu `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace --locked`, dan `cargo deny`.
  - Pada run terakhir, `generate` melaporkan kedua file **`unchanged`**, artinya file yang di-*check in* sama dengan output generator.
  - **70 tes lulus, 0 gagal.** Satu doc-test `ignore` adalah contoh pemakaian Tauri.
  - `cargo deny check licenses bans sources`: **`bans ok, licenses ok, sources ok`**. Satu peringatan `duplicate` untuk `syn` 2/3 berasal dari crate pihak ketiga dan diizinkan (`multiple-versions = "warn"`).
  - `cargo deny --offline check advisories`: **`advisories ok`**.
  - Job selesai dengan **exit code 0** (22 Okt 2026 19:06).
- **Riwayat run:**
  - Run pertama menemukan dua bug yang sudah diperbaiki:
    - *overflow* penyebut pada property test (§9 butir 0);
    - `*/` di doc comment yang merusak `bindings.ts` (§2.2).
  - Sandbox tidak punya `git`, jadi `cargo deny check advisories` memakai salinan basis data RustSec milik host (`--offline`, snapshot 17 Okt 2026).
- **Host (tanpa eksekusi biner):**
  - `tsc --strict` (TypeScript 6.0.3) atas `bindings.ts` + harness pemeriksaan: **0 galat**. Uji negatif `blendMode: "dodge"` ditolak.
  - `eslint bindings.ts`: **0 masalah**.
  - `prettier --check` atas skema dan fixture: **lolos**.

## 7. Cara membangkitkan ulang

```powershell
cargo run -p kinora-ipc --example generate   # menulis bindings.ts dan schema/project.v0.json
cargo test -p kinora-ipc -p kinora-project   # memastikan keduanya mutakhir
```

Di mesin dev dengan Smart App Control, jalankan lewat sandbox: `scripts/sandbox/jobs/ipc-contract.ps1` (generate → fmt → clippy → test → cargo-deny).

- `bindings.ts` dibandingkan **byte-per-byte** (CRLF dinormalisasi).
- Skema dibandingkan sebagai **nilai JSON**, sehingga boleh diformat Prettier. Generator tidak menulis ulang file yang nilainya sama. Jika skema berubah, jalankan `pnpm exec prettier --write crates/kinora-project/schema` setelah generate.
- Header file TS menyebut perintah regenerasi.

## 8. Usulan edit file bersama (tidak saya ubah)

### 8.1 `.github/workflows/ci.yml`
`cargo test --workspace --locked` yang sudah ada **sudah gagal** bila binding/skema basi. Untuk pesan yang jelas, tambahkan di job `rust` setelah langkah test:

```yaml
      - name: Generated IPC bindings and project schema are current
        run: |
          cargo run -p kinora-ipc --example generate --locked
          git diff --exit-code -- apps/desktop/ui/src/lib/ipc/bindings.ts crates/kinora-project/schema
```

### 8.2 `package.json` (scripts) dan `.prettierignore`

```json
"ipc:generate": "cargo run -p kinora-ipc --example generate",
"ipc:check": "cargo test -p kinora-ipc --test contract && cargo test -p kinora-project --test document"
```

`.prettierignore` — **wajib** sebelum commit, karena tanpa ini `pnpm run format:check` gagal pada `bindings.ts` (format bawaan `ts-rs` berbeda dari Prettier dan file dibandingkan byte-per-byte). Skema JSON **tidak** perlu diabaikan: file yang di-*check in* sudah diformat Prettier, sedangkan tes dan generator membandingkannya sebagai nilai JSON.

```
# Generated by crates/kinora-ipc (cargo run -p kinora-ipc --example generate).
apps/desktop/ui/src/lib/ipc/bindings.ts
```

ESLint (konfigurasi saat ini) lolos pada `bindings.ts`; tidak perlu diubah.

Setelah UI di-*scaffold* (P1, ada `tsconfig`), tambahkan cek tipe permanen `apps/desktop/ui/src/lib/ipc/contract.test-d.ts` yang mengimpor fixture sebagai literal dengan `satisfies Project`, ditambah kasus `@ts-expect-error`. Harness yang saya pakai untuk verifikasi manual (§6) bisa dijadikan dasar. Pasangkan dengan skrip `"ipc:typecheck": "tsc -p apps/desktop/ui --noEmit"` di job `web` CI.

### 8.2b `scripts/sandbox/README.md`
Di bagian *Jobs*, tambahkan: "[`jobs/ipc-contract.ps1`](jobs/ipc-contract.ps1) membangkitkan ulang binding TS dan skema `project.json`, lalu menjalankan fmt, clippy, test, dan `cargo deny` (advisories memakai basis data RustSec host secara offline)."

### 8.3 `docs/03-master-plan.md` dan `docs/04-phase0-plan.md`
- 03 §5.1: tambahkan baris `kinora-ipc : kontrak IPC bertipe (command, event, galat) + generator binding TS`; ganti `"project.changed" / "clock.tick"` → `"project:changed" / "clock:tick"`.
- 03 §5.2: `timelines[]` tetap; tambahkan "ID UUIDv7 per entitas; waktu rasional `{num, den}`".
- 03 §5.5: `crates/` menyebut `-ipc`.
- 04 baris P0-12: tandai selesai, tautkan laporan ini.

### 8.4 Amandemen ADR-0004 (teks usulan)

> ## Amandemen 1 — Kontrak tipe & format v0 (21 Okt 2026, P0-12)
> 1. **Waktu** tetap `{num: i64, den: u32}`, dengan tambahan: `|num| ≤ 2^53 − 1` (aman untuk JavaScript), selalu dalam bentuk terkecil, aritmetika *checked* (tanpa pembulatan diam-diam). Diimplementasikan di `kinora-core`.
> 2. **`schemaVersion`** (camelCase, seperti seluruh JSON) adalah **integer** yang naik pada setiap perubahan format, bukan semver. Build lama menolak dokumen yang lebih baru. Dokumen lama dimigrasikan maju per langkah pada JSON tak bertipe. Versi 0 = draf dan dibekukan menjadi 1 pada rilis pertama yang menulis proyek pengguna.
> 3. **Parsing ketat**: field dan varian tak dikenal ditolak. Kompatibilitas maju hanya lewat gerbang versi.
> 4. **ID** entitas = UUIDv7, dibuat pemanggil dan dibawa dalam command.
> 5. **Command** dokumen bernama `EditCommand` di kode, dengan bentuk kabel `{op, params}`.
> 6. **Skema publik** dibangkitkan `schemars` ke `crates/kinora-project/schema/project.v<N>.json`. **Tipe TypeScript** dibangkitkan `ts-rs` (bukan dari skema) ke `apps/desktop/ui/src/lib/ipc/bindings.ts` bersama kontrak IPC dari `kinora-ipc`. Keduanya diuji *drift* di CI.
> 7. **Fixture** per versi berada di `crates/kinora-project/tests/fixtures/v<N>/` (menggantikan `tests/fixtures/projects/`).

## 9. Pertanyaan terbuka
0. **Timebase untuk edit (temuan property test).** Menjumlahkan waktu dengan penyebut koprima (mis. `1/1633 + 805774/3051`) membuat penyebut melewati `u32::MAX`. Aritmetika mengembalikan `None` dan validator menolak dokumen, jadi **tidak ada korupsi**, tetapi edit itu gagal. Media nyata memakai sedikit timebase (frame rate, sample rate, 90 kHz), sehingga property test kini memakai timebase tersebut. Usulan untuk inti edit P1: setiap posisi hasil edit di-*snap* ke batas frame timeline, atau ke satu *tick* global. Kandidat tick: *flick* = 1/705.600.000 s, yang habis membagi 24/25/30/48/50/60/90/100/120 fps, 23,976/29,97/59,94 fps, dan 8–192 kHz, serta muat di `u32`. Dengan begitu penyebut tidak pernah tumbuh. Perlu keputusan sebelum `apply` diimplementasikan.
1. **Branded ID di TS** (`string & {__brand: 'ClipId'}`) mencegah tertukar di UI, tetapi butuh cast saat UI membuat ID. Aktifkan di P1?
2. **Delta pada `project:changed`**: v0 menarik dokumen penuh (`get_project`) setelah tiap edit. Untuk proyek besar dan *drag* interaktif, perlu *patch* (RFC 6902 atau delta per entitas)? Ukur di P1.
3. **Format jurnal** (`journal.jsonl`): tiap baris `{schemaVersion, revision, command}`? Belum didefinisikan.
4. **Membuka dokumen versi lebih baru secara *read-only*** alih-alih menolak sepenuhnya?
5. **Kurva kecepatan, mask, compound clip, grup/link, font proyek, brand kit**: ditunda ke versi skema berikutnya (P2/P4). Struktur enum bertag sudah menyiapkan tempatnya.
6. **Pembaruan specta**: jika `specta` 2.0 stabil dan `f64 → number` bisa dikonfigurasi, evaluasi ulang `tauri-specta` untuk shell (kontrak tetap di `kinora-ipc`).
7. **Fuzzing parser** (`from_json`, `migrate`): CLAUDE.md meminta parser input tak tepercaya di-*fuzz*. Usulan: target `cargo-fuzz` di P1, karena butuh toolchain nightly + sanitizer yang tidak berjalan di host SAC. Saat ini sudah ada batas ukuran 64 MiB, batas rekursi bawaan `serde_json` (128), dan property test.
8. **`clock:tick`** (ADR-0003 §4): S1 memakai *pull* (`clock` command). Tentukan apakah event ini masih diperlukan sebelum ditambahkan ke kontrak.
