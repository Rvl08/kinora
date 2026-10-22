---
version: 1
slug: "src-lib-features-start-startscreen-svelte"
primary_target: "src/lib/features/start/StartScreen.svelte"
related_targets: []
---

# Start screen

## Scope

The first screen of the Kinora desktop app (Operate). It is shown with no project open. The visual world chosen
here (22 Oct 2026) is the app-wide world; later surfaces extend it.

## Audience and task

- **Who:** a first-time short-form creator on a 1366×768 laptop, often editing at night.
- **Job:** start a vertical video in one action (a ratio preset, or dropping a video), or reopen a recent project.
- **Constraints:**
  - two clicks at most;
  - Indonesian and English;
  - keyboard and screen-reader complete;
  - WCAG AA in dark and light themes.
- **States:** no recent projects (first run); some projects; a recent project whose folder is missing; dragging a
  file over the window.

## Direction contract

THESIS: Your projects come back to you the way prints came back from the neighbourhood *cuci cetak* counter: a
contact sheet you can read at a glance. It refuses the category default, a grey dashboard of identical project
cards with a blue accent.

OWN-WORLD:
- **Palette:** a warm film-rebate charcoal ground, print-white type, and the orange of colour-negative base as the
  one committed accent (the primary action, selection, the playhead). Safelight red is for errors and safelight
  amber for work in progress.
- **Surfaces:** flat and matte, with no gradients or glow.
- **Type:** condensed caps for edge-print labels, with tabular frame numbers. Body text uses the system UI face.
- **Prints:** thumbnails sit in thin white print borders and are never tinted; all share one scale and one
  baseline.
- **Ruling:** order-form hairlines separate the fields.

STORY: The visitor sees at once how to start (drop a video, or pick 9:16) and sees past work as a contact sheet
with honest, right-aligned durations. Opening one takes a single click. Platform duration limits are left out:
they change, and the product must not state them as fact.

FIRST VIEWPORT:
- **Left column (about 38%):** the Kinora mark; the heading "Proyek baru"; three ratio buttons, each shaped as its
  own frame (9:16 first and primary, then 16:9 and 1:1); below them a drop band, "Seret video ke sini"; and
  "Buka proyek…" as the secondary action.
- **Right column:** "Terakhir dibuka" as a contact sheet. Rows of prints, each over an edge-print strip:
  frame-style number · ratio · duration (right-aligned, tabular) · date.
- **Empty state:** an unexposed strip carrying the first step.
- **Signature move:** the edge-print strip, reused later as the timeline ruler.

FORM: Impeccable's pick (my top-ranked grounded candidate, #1 on the ordered list: *cuci cetak* contact sheets
and print envelopes), chosen by the owner over the assigned direction. seed:375382b4c9f2f954

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Unresolved

- Project thumbnails need a shell-side thumbnail cache (Phase 1 shell).
- Until it exists, prints show an unexposed frame in the project's ratio.
