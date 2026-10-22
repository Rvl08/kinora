# CLAUDE.md — working rules for Kinora

Read this first in every session, local or cloud. The owner writes in Bahasa Indonesia; reply in Indonesian
unless asked otherwise. The detailed plans live in `docs/` (Indonesian); this file is the short contract.

## 1. What Kinora is

A free, offline, lightweight **Windows** video editor: CapCut-class editing and on-device AI, with no watermark,
account, subscription or upload.

- **Stack:** Tauri 2 + Svelte 5 (TypeScript) + Rust workspace; FFmpeg 8.1 **LGPL**; wgpu/WGSL effects; preview
  through WebCodecs + WebGPU; AI through ONNX Runtime + DirectML and whisper.cpp (Vulkan).
- **Licence:** MIT OR Apache-2.0 (D1). Repository: <https://github.com/Rvl08/kinora>.
- **Distribution:** Microsoft Store (MSIX) and a signed NSIS `setup.exe`. Both are free, see §6.
- **Key docs:**
  - `docs/03-master-plan.md`: vision, architecture, §6.4 performance budget, §6.6 UI rules, §7 roadmap.
  - `docs/04-phase0-plan.md`: Phase 0 backlog, with the progress table in §3.1.
  - `docs/adr/` (0001–0007): binding decisions. Change one only through an amendment.
  - `docs/06-public-release-plan.md`: zero-cost distribution.
  - `docs/07-frontend-guidelines.md`: binding UI architecture and design rules for `apps/desktop/ui`.

## 2. Status (22 Oct 2026). The source of truth is `docs/04-phase0-plan.md` §3.1

| Done | Notes |
|---|---|
| P0-01 name | "Kinora" kept (owner's decision; no formal trademark check) |
| P0-02/03 repo, toolchain | Rust 1.99.1 pinned, pnpm via Corepack, ESLint/Prettier |
| P0-04 CI | `.github/workflows/ci.yml` green on GitHub |
| P0-05 FFmpeg dev build | BtbN LGPL shared build in `vendor/` (dev only) |
| P0-06 signing pipeline | `scripts/signing/` (sign, verify gate, policy). Free providers not enrolled yet |
| P0-07 S1 preview | ADR-0003 accepted for discrete GPUs |
| P0-08 S2 export | NVENC paths and the component list for the release FFmpeg |
| P0-09 S3 AI runtime, P0-09b S3b cutout | ADR-0006: whisper Vulkan; Kokoro without espeak-ng; BiRefNet dynamic export |
| P0-11 fixtures | `scripts/fixtures/`, 15 freely licensed fixtures (media outside git) |
| P0-12 IPC contract | `crates/kinora-project` (project.json v0) and `crates/kinora-ipc`; ts-rs bindings in `apps/desktop/ui/src/lib/ipc/bindings.ts` and the JSON Schema are generated (`pnpm run ipc:generate`) and drift-tested |
| Editing core (pre-Phase 1) | ADR-0004 amendment 2: timeline times on a **flick grid** (1/705,600,000 s). `kinora-project::edit`: atomic `apply`, exact undo/redo patches, `Editor` session, `journal.jsonl` crash recovery. Linux-tested; Windows sandbox run pending (`docs/spikes/edit-core-report.md`) |

**In progress or not done:**
- **P0-05b:** static release FFmpeg built in CI (`scripts/ffmpeg/release*`, `.github/workflows/ffmpeg-release.yml`).
  Check `git log` and the docs/04 table before touching it: it may already be committed.
- **P0-10 / Spike S4:** NSIS installer with wizard + MSIX (winapp CLI); test install in a clean Windows Sandbox.
- **P0-13:** Phase 0 go/no-go gate, then the Phase 1 backlog.
- **P0-14:** public channels. Register with SignPath Foundation (needs a first release and the "Code signing
  policy" section) and the Microsoft Store. The owner does the registrations.
- **Untested hardware:** iGPU (Intel/AMD), QSV/AMF encoders, 8 GB laptops.
- **Phase 1 (MVP editor) has not started**, apart from the editing core above. Its first UI step is setting up
  Impeccable (§4). The Tauri shell must write the journal (header on open/save, append and sync each entry,
  atomic save); see the report's §3.3.

## 3. Non-negotiable rules

- **Zero cost.** No paid service, certificate or subscription. Use free programmes and design around their
  limits (example: SignPath signs only our own artifacts, so FFmpeg is linked statically).
- **Licences.**
  - Only permissive components (MIT, BSD, Apache, ISC, zlib, CC0/CC-BY data). LGPL is allowed only for FFmpeg.
  - Never GPL/AGPL, non-commercial (NC), or "research only".
  - Banned by name: x264, x265, libvidstab, espeak-ng, RVM, F5-TTS, NLLB, SCRFD, RMBG (BRIA).
  - `cargo deny`, `pnpm run licenses:check` and `pnpm run ffmpeg:verify` must pass.
- **Clean-room.** Never copy, port or translate code, shaders, presets, UI text, assets, string tables or models
  from CapCut or Concat (AGPL). Feature ideas come from `docs/02-feature-matrix.md` only. The internal
  CapCut/Concat analysis (`docs/01`) is private: **never recreate it, quote it, or add it to this repository.**
- **Git.**
  - Commit identity: `Rvl08 <150601118+Rvl08@users.noreply.github.com>`. Always `git commit -s` (DCO; CI checks it on pull requests).
  - Conventional subjects (`feat(scope): …`, `fix`, `docs`, `build`, `test`, `spike`), with a body that explains why.
  - Never put the owner's private email or real name anywhere.
  - **Local sessions never push; the owner pushes.**
  - **Cloud sessions** may push only their own work branch (`claude/...` or `wip/...`), never `main`. No force-push, no merging, no rewriting history.
- **Privacy and security.**
  - No telemetry; ONNX Runtime telemetry stays off.
  - Every download is pinned (URL plus SHA-256, and for third-party binaries an Authenticode check) and verified before use.
  - Never disable or work around Windows security features (Smart App Control, Defender, UAC).
  - Never commit secrets, tokens, keys or `.pfx` files.

## 4. Quality bar (owner's explicit request)

**Production-grade on every layer:** clean, maintainable, scalable, robust; performance, security and safety
first.

**Frontend (Svelte 5 + TS)**
- **Always use the Impeccable skill for any UI or design work**: `npx impeccable install`, then
  `/impeccable init` and `/impeccable hooks on` (<https://impeccable.style>). The hooks need the owner's approval.
- The goal is clean, elegant, professional and easy for anyone. **No "AI slop":** no generic purple gradients,
  identical cards or decorative icons.
- Follow `docs/07-frontend-guidelines.md` (structure, read-model data flow, i18n, tokens, a11y, testing) and docs/03 §6.6:
  - progressive disclosure, and ≤ 2 clicks for common actions;
  - honest progress for AI jobs, which can always be cancelled;
  - dark and light themes; WCAG AA contrast; keyboard-first for pro users;
  - natural Indonesian and English UI text (i18n from day one).
- TypeScript in strict mode, Svelte 5 runes, no `any`. Components stay small and testable.
- Never push large payloads (frames, audio) over IPC (ADR-0003).

**Backend (Rust)**
- Workspace lints apply (`missing_docs`, no `unwrap` in library code, `undocumented_unsafe_blocks` denied).
  CI runs `cargo clippy -D warnings`.
- Typed errors (`thiserror`). Errors cross IPC as safe, serialisable values: no panics, no internal paths.
- Every project change is a **Command** (undo/redo, journal). The UI, Copilot, MCP and scripts all go through it (ADR-0004).
- Timeline time is **rational**, never floating point, and on the flick grid (`Time::is_on_grid`); round
  explicitly with `scale_to_grid`/`round_to_grid`, never implicitly.
- Treat media, project files, templates and effect packages as **untrusted input**: bounds checks, no unbounded allocation, fuzz the parsers.

**Data and schema**
- `project.json` carries a `schemaVersion`, stable IDs, and forward migrations with tests. The JSON Schema is
  generated from the Rust types, and the TS bindings are generated, never hand-edited.

**Performance:** respect docs/03 §6.4:
- installer ≤ 120 MB; cold start ≤ 2 s; idle RAM ≤ 300 MB;
- 1080p preview ≥ 30 fps on iGPU; export ≥ 2× realtime.

**Tests:** unit and property tests for time, commands and undo; golden-frame tests for rendering; and a
regression test for each bug fixed.

**Docs:** `docs/` in Bahasa Indonesia; code, comments and READMEs in code folders in English. A significant
decision means a new ADR or an amendment, plus an update to the docs/04 progress table.

## 5. Environments: what can run where

| | **Local: Claude Desktop on the owner's Windows 11** | **Cloud: Claude Code on the web (Linux container)** |
|---|---|---|
| Good for | MSVC and FFmpeg builds, GPU work (NVENC, DirectML, Vulkan), WebView2, installer, signing, **all final testing** | Platform-independent Rust crates (`kinora-project`, `kinora-core`, IPC contract, timeline logic), Svelte/TS UI with Impeccable, docs, CI workflows, Node scripts |
| Cannot | Run freshly built executables directly: **Smart App Control** blocks them (os error 4551) | Build or run Windows binaries, test GPUs/WebView2/the installer, use Windows Sandbox. Large downloads may be blocked |
| How | Native builds and tests go through `scripts/sandbox/invoke.ps1` (queued Windows Sandbox; read `scripts/sandbox/README.md`) | `cargo test -p <crate>` for portable crates; `pnpm` lint/format/test |

**Cloud session rules:**
- Set the git identity first:
  `git config user.name Rvl08 && git config user.email 150601118+Rvl08@users.noreply.github.com`.
- Work on a branch and keep commits small, each signed off with `-s`.
- Do not edit `vendor/`, `models/cache`, `tests/media` or Windows-only scripts you cannot test.
- When work needs Windows verification, say so and leave clear test steps (e.g. which Sandbox job to run).
  Never claim it was tested if it was not.
- Don't download model weights or large media; use small synthetic data in tests.
- Code that compiles only on Windows needs `#[cfg(windows)]` with a portable stub or fallback, so the
  portable crates still test on Linux.

## 6. Distribution and signing (ADR-0007 amendment 1)

- Every shipped exe and dll is signed or vendor-signed (Microsoft, Cisco).
- Release gate: `pnpm run sign:verify <dir>`.
- Release layout: no unsigned third-party DLLs.
  - FFmpeg and ggml are linked statically (ADR-0005 amendment 3, ADR-0006 amendment 1).
  - DirectML and Cisco OpenH264 are downloaded at run time, verified, and already signed by their vendors.
- Channels: the Microsoft Store re-signs the MSIX package. SignPath Foundation signs `setup.exe`, once approved;
  until then preview builds are unsigned and say so.

## 7. Commands

```sh
corepack pnpm install
corepack pnpm run lint && corepack pnpm run format:check && corepack pnpm run licenses:check
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo deny check
corepack pnpm run ffmpeg:fetch | ffmpeg:verify          # dev FFmpeg (Windows)
corepack pnpm run fixtures:fetch | fixtures:generate | fixtures:verify
corepack pnpm run sign | sign:verify                    # see scripts/signing
powershell -ExecutionPolicy Bypass -File scripts/sandbox/invoke.ps1 -Name rust-check -Script scripts/sandbox/jobs/rust-check.ps1   # Windows host
```

## 8. Never

- Never push to `main`; never push at all from a local session.
- Never add a paid dependency or service.
- Never add GPL/AGPL/NC code, models or media.
- Never copy from CapCut or Concat, or publish `docs/01`.
- Never disable Smart App Control or other Windows security.
- Never hand-edit generated files (TS bindings, JSON Schema).
- Never put secrets or the owner's personal data in the repository.
- Never claim untested work is tested; never skip `-s` or the CI checks.
- Never ship UI work without Impeccable.
