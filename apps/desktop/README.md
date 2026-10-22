# apps/desktop

The Tauri 2 desktop application.

- `src-tauri/`: Rust glue between the window and the engine crates (commands, IPC channels, bundling
  config). Not created yet; it arrives with the Phase 1 shell work.
- `ui/`: Svelte 5 + TypeScript front end (panels, timeline canvas, inspector, WebGPU preview). Rules:
  [docs/07-frontend-guidelines.md](../../docs/07-frontend-guidelines.md).

## ui

The start screen is built (visual world "cuci cetak", designed with Impeccable). The design system is recorded in
[ui/DESIGN.md](ui/DESIGN.md) and the product brief in [ui/PRODUCT.md](ui/PRODUCT.md); read both before UI work.

| Path | What |
|---|---|
| `src/lib/ipc/` | `bindings.ts` (generated from Rust, never edit), typed `IpcClient`, `Transport` (+ `MemoryTransport` for tests), error → message mapping, a dev-only demo shell (`demo.ts`) |
| `src/lib/state/` | Read-models of the shell: `ProjectStore` (commands with `baseRevision`, pull on `project:changed`), `RecentProjectsStore` |
| `src/lib/i18n/` | Typed dictionaries (Indonesian source, English), `t()`, plurals, relative dates, language detection |
| `src/lib/time/` | Exact rational time ↔ frames, timecode and durations for display |
| `src/lib/theme/` | Design tokens (`tokens.css`, OKLCH, dark + light), theme selection (`<html data-theme>`), the bundled display font |
| `src/lib/components/` | Domain-free primitives (`Button`) |
| `src/lib/features/` | `start/` (start screen: ratio presets, drop band, recent-project contact sheet), `settings/` (language and theme) |

```sh
pnpm run ui:check   # svelte-check + tsc for the Vite config
pnpm run ui:test    # Vitest
pnpm run ui:build   # static build in ui/dist for the Tauri shell
pnpm --filter @kinora/ui run dev   # browser preview at http://localhost:1420
```

Without the Tauri shell, the dev server answers IPC calls from the demo shell with sample data marked "(contoh)".
Add `?state=empty`, `?state=error` or `?state=loading` to the URL to see the other states. A production build has
no shell yet and shows the error state.
