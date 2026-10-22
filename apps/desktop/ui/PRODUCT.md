# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The UI is web technology (Svelte 5 in Microsoft WebView2) hosted by a Tauri 2 desktop shell on Windows 10/11. It
ships as a desktop application, not a website: one window, no URL routing, and it should feel at home among native
Windows desktop software.

## Users

- **Primary (sets the defaults): first-time short-form creators.** People making TikTok, Reels and Shorts videos,
  often on an ordinary laptop, with little or no editing experience. Their job: turn phone footage into a vertical
  9:16 video with text and music, and export it, in under 10 minutes without help (Phase 1 exit criterion).
- Supported through progressive disclosure, not the defaults:
  - educators and podcasters (transcript-first editing, filler-word removal; mostly Phase 3);
  - small businesses and marketers (templates, brand kit, multi-ratio export);
  - professional editors and developers (keyboard-first work, scripting, MCP, NLE interchange).
- Indonesian speakers are a first-class audience. Indonesian is the source language of the UI; English is
  complete too.

## Product Purpose

Kinora is a free, offline, lightweight video editor for Windows, with CapCut-class editing and on-device AI. It has
no watermark, account, subscription, credits or uploads. Success means a beginner can finish and export a good
short video quickly, and a professional can work fast without the tool getting in the way.

## Positioning

- Everything runs locally and stays free: AI features (captions, cutout, noise reduction, TTS) are never locked behind
  payment or an account, and nothing is uploaded.
- Transcript-first editing and one-click speech cleanup, including Indonesian filler words.
- A local AI Copilot and an MCP server that edit through the same undoable commands as the UI.
- Indonesian as a first-class language, open project formats, and NLE interchange.

## Operating Context

- **Hardware:** Windows 10 22H2 or 11 laptops and desktops. The minimum is 8 GB RAM and an integrated GPU. The
  smallest screen that must work comfortably is **1366×768**; larger screens get more room, not a different
  layout.
- **Workflow:** import media → arrange clips on a multi-track timeline (split, trim, ripple, snap) → add text,
  captions, music and effects → watch the preview → export (H.264/HEVC presets for platforms).
- **Long-running jobs** (export, proxies, transcription, AI) run in the background with honest progress and can
  always be cancelled.
- Projects are folders (`Name.kinora/`) with autosave and crash recovery. Every edit can be undone.

## Capabilities and Constraints

- **Capabilities:** Phase 1 (MVP) covers import and media bin, proxies, a multi-track timeline, undo/redo with crash
  recovery, preview, transform/crop/rotate/opacity, basic text and fonts, volume/fade/waveform, export up to 4K,
  and ID/EN UI.
  - The editing core is built in Rust: 28 undoable operations, revisions and a journal.
  - Later phases add keyframes, effects, transitions, colour and AI features (docs/02, docs/03 §7).
- **Technical constraints:**
  - The UI is a read-model of the Rust editor and never mutates the document itself.
  - Large data (frames, audio) never travels over IPC.
  - Performance budgets: cold start ≤ 2 s, idle RAM ≤ 300 MB, 1080p preview ≥ 30 fps on an iGPU.
  - The timeline is drawn on canvas.
- **Zero cost and licences:** only permissive dependencies; no telemetry; no remote fonts or assets at runtime.
- **Clean-room:** never copy UI text, layouts, presets or assets from CapCut or Concat.
- **Terminology:** creators' everyday terms stay as they are in Indonesian UI text (klip, timeline, track, ekspor,
  caption).

## Brand Commitments

- The name is **Kinora**. No logo, colours or app icon exist yet; the visual identity is open and is to be designed.
- Tone: calm, clear and professional, with natural Indonesian rather than stiff translation. Explicitly not
  "AI slop": no generic purple gradients, identical cards or decorative icons.

## Evidence on Hand

- A sample project: `crates/kinora-project/tests/fixtures/v0/sample.kinora/project.json`.
- Test media fixtures (outside git; `pnpm run fixtures:fetch`).
- No users, testimonials, benchmarks against competitors, or press exist yet; do not invent any.

## Product Principles

1. **The content is the star.** Preview and timeline get the space and contrast; the chrome stays quiet.
2. **Simple first, power one step away.** Common actions take ≤ 2 clicks and have shortcuts; advanced controls sit
   behind progressive disclosure.
3. **Honest and reversible.** Every edit can be undone, jobs show real progress and can be cancelled, and errors say
   what happened and what to do next.
4. **Respect the machine.** It stays fast and light on an 8 GB laptop with an integrated GPU; nothing heavy loads
   until it is needed.
5. **Local and private by default.** No account, no upload, no tracking.

## Accessibility & Inclusion

- WCAG 2.2 AA in both light and dark themes.
- Full keyboard operation with visible focus, and screen-reader labels (Narrator).
- Respects `prefers-reduced-motion`; never uses colour alone to carry meaning.
- Indonesian and English throughout.
