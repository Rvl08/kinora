# Kinora

> **Working name.** The final product name is decided in Phase 0 (task P0-01).

A free, offline, lightweight desktop video editor for Windows: the editing and on-device AI people open CapCut for,
with **no watermark, no account, no subscription, no upload**, installed through a single signed `setup.exe`.

**Status:** Phase 0 (foundations and de-risking). No product code yet. See [`docs/`](docs/README.md).

## Highlights (planned)

- Multi-track timeline, keyframes, speed curves, masks, chroma key, color grading, LUTs
- Local AI: auto captions, transcript-based editing, filler-word and pause removal, background removal,
  noise reduction, voice isolation, text-to-speech, auto reframe, upscaling
- AI Copilot (local LLM or bring-your-own-key) and a built-in MCP server, both driving the same edit commands as the UI
- Bahasa Indonesia as a first-class language
- Open project format; export to FCPXML / OTIO / EDL / SRT

## Repository layout

| Path | Contents |
|---|---|
| `apps/desktop/` | Tauri 2 shell (`src-tauri`) and Svelte 5 UI (`ui`) |
| `crates/` | Rust engine crates (`kinora-*`) |
| `effects/` | Effect packages: `effect.toml` + `effect.wgsl` + fixtures |
| `assets/` | Bundled open-license fonts, stickers, music |
| `models/` | `manifest.toml`: AI models downloaded on demand, with SHA-256 and licence |
| `installer/` | NSIS installer configuration |
| `tests/golden/` | Golden-frame test projects and reference frames |
| `spikes/` | Throwaway Phase 0 prototypes (never shipped) |
| `docs/` | Planning documents (Bahasa Indonesia) and Architecture Decision Records |

## Development

Prerequisites (developers only; end users just run the installer):

- Rust: pinned by [`rust-toolchain.toml`](rust-toolchain.toml); run `rustup toolchain install` in the repository root
- Node.js 24 ([`.nvmrc`](.nvmrc)) with Corepack, which provides the pinned pnpm
- Visual Studio Build Tools (MSVC) and the Windows SDK
- An **LGPL** FFmpeg 8.1 shared build in `vendor/ffmpeg-lgpl`: run `corepack pnpm run ffmpeg:fetch`, which downloads the
  build pinned in [`scripts/ffmpeg/lock.json`](scripts/ffmpeg/lock.json), checks its SHA-256 and verifies its licence
  (see [ADR-0005](docs/adr/0005-codec-and-ffmpeg-policy.md)). `.cargo/config.toml` points `FFMPEG_DIR` there.

```sh
corepack pnpm install        # JS tooling
cargo fmt --all --check      # Rust formatting
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
corepack pnpm run lint       # ESLint
corepack pnpm run format:check
corepack pnpm run licenses:check   # npm licence policy
cargo deny check             # Rust licence, source and advisory policy (cargo install cargo-deny)
```

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs the same checks, plus actionlint and a DCO sign-off check on pull requests.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project
shall be dual licensed as above, without any additional terms or conditions. See [CONTRIBUTING.md](CONTRIBUTING.md).
