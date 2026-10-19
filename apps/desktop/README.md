# apps/desktop

The Tauri 2 desktop application.

- `src-tauri/`: Rust glue between the window and the engine crates (commands, IPC channels, bundling config)
- `ui/`: Svelte 5 + TypeScript front end (panels, timeline canvas, inspector, WebGPU preview)

Scaffolded after Phase 0 Spike S1 settles the preview pipeline ([ADR-0003](../../docs/adr/0003-preview-pipeline.md)).
