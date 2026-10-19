# Contributing to Kinora

Thank you for helping. This page covers the rules every change follows.

## 1. Clean-room rule (mandatory)

Kinora is an independent implementation. It is **not** derived from CapCut or from Concat.

- Do not copy, port or translate code, shaders, effect presets, UI text, documentation or assets from
  CapCut, Concat (AGPL-3.0) or any other editor whose licence is not in the allowed list.
- Do not use CapCut binaries, models, fonts, stickers, LUTs, string tables, names or icons.
- Feature ideas and observable behaviour are fine; the shared knowledge is in
  [`docs/01`](docs/01-reverse-engineering-report.md) and [`docs/02`](docs/02-feature-matrix.md).
- While implementing an effect or feature, do not open the source code of a GPL/AGPL editor that implements it.

Every pull request confirms this in its template.

## 2. Licences of what you add

New dependencies, models and assets must be in the allowed classes of
[`docs/05-license-compliance.md`](docs/05-license-compliance.md). CI enforces this. GPL, AGPL and
non-commercial licences are rejected.

## 3. Developer Certificate of Origin

Every commit must be signed off, certifying the [Developer Certificate of Origin 1.1](https://developercertificate.org/):

```sh
git commit -s -m "feat(timeline): ripple delete"
```

Contributions are dual licensed under MIT OR Apache-2.0, as stated in the [README](README.md#license).

## 4. Before opening a pull request

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
corepack pnpm run lint
corepack pnpm run format:check
corepack pnpm run licenses:check
cargo deny check
```

## 5. Conventions

- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/) (`feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `chore`), with the crate or area as scope.
- **Time:** timestamps in the project model are rational seconds, never `f64` ([ADR-0004](docs/adr/0004-project-format-and-commands.md)).
- **Edits:** every change to a project is a `Command`; the UI, the Copilot and the API never mutate the document directly.
- **Architecture changes:** propose an ADR in `docs/adr/` before writing the code.
- **Language:** code, comments, commit messages and repository files in English; planning documents in `docs/` are in Bahasa Indonesia.
