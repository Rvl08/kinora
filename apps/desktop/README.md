# apps/desktop

The Tauri 2 desktop application.

- `src-tauri/`: Rust glue between the window and the engine crates (commands, IPC channels, bundling
  config). Not created yet; it arrives with the Phase 1 shell work.
- `ui/`: Svelte 5 + TypeScript front end (panels, timeline canvas, inspector, WebGPU preview). Rules:
  [docs/07-frontend-guidelines.md](../../docs/07-frontend-guidelines.md).

## ui

Foundation only, no visual design yet (that is done with Impeccable):

| Path | What |
|---|---|
| `src/lib/ipc/` | `bindings.ts` (generated from Rust, never edit), typed `IpcClient`, `Transport` (+ `MemoryTransport` for tests), error → message mapping |
| `src/lib/state/` | `ProjectStore`: the read-model of the shell's editor (commands with `baseRevision`, pull on `project:changed`) |
| `src/lib/i18n/` | Typed dictionaries (Indonesian source, English), `t()`, plurals, language detection |
| `src/lib/time/` | Exact rational time ↔ frames, timecode and durations for display |
| `src/lib/theme/` | Light / dark / system theme selection (`<html data-theme>`) |

```sh
pnpm run ui:check   # svelte-check + tsc for the Vite config
pnpm run ui:test    # Vitest
pnpm run ui:build   # static build in ui/dist for the Tauri shell
pnpm --filter @kinora/ui run dev   # browser preview at http://localhost:1420 (no shell: IPC unavailable)
```
