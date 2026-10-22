# .claude — Claude Code project configuration

## Impeccable (design skill)

`CLAUDE.md` §4 requires the [Impeccable](https://impeccable.style) skill for all UI work. Installed here at project
scope, pinned:

| Item | Version | Source |
|---|---|---|
| npm launcher | `impeccable@4.7.0` (Apache-2.0) | npmjs.com |
| Skill + agents | 4.7.0 (`skills/impeccable/SKILL.md`) | impeccable.style bundle, via the launcher |
| Engine binary | 0.1.16 (`skills/impeccable/scripts/VERSION`) | GitHub releases `pbakaus/impeccable`, SHA-256 sidecar verified |

- `skills/impeccable/` and `agents/impeccable-*.md` are committed so every session uses the same version. The
  licence is in `skills/impeccable/LICENSE`.
- The engine binary (`skills/impeccable/scripts/bin/`) is **not** committed: it is platform-specific and about
  20 MB. The launcher `skills/impeccable/scripts/impeccable` (or `impeccable.cmd` on Windows) downloads it on first
  use and verifies its SHA-256 before running it.
- Hooks (detector after every UI edit, deep pass at the end of a turn) were approved by the owner (22 Oct 2026). They
  live in the per-machine `settings.local.json`, which is not committed. Project settings are in
  `../.impeccable/config.json`.

To set up a new machine or cloud session:

```sh
npx --yes impeccable@4.7.0 install -y --providers=claude --scope=project
.claude/skills/impeccable/scripts/impeccable hooks on
```

Then check that `git status` shows no changes under `skills/` (same version).

**Windows (local) note:** the downloaded engine is not Authenticode-signed, so Smart App Control may block it. Do
not disable Smart App Control (`CLAUDE.md` §3). Run design checks from a cloud session, or through the Windows
Sandbox runner, instead.

Product context for the skill: `apps/desktop/ui/PRODUCT.md`. Binding UI rules: `docs/07-frontend-guidelines.md`.
