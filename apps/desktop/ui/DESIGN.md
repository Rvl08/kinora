---
name: Kinora
description: A free, offline Windows video editor whose UI is a neighbourhood photo lab ("cuci cetak"): darkroom by night, photo paper by day.
colors:
  # Dark theme (the darkroom; default when the system asks for dark). Source: src/lib/theme/tokens.css
  surface-base: "oklch(0.205 0.011 55)"
  surface-sunken: "oklch(0.175 0.01 55)"
  surface-print: "oklch(0.78 0.01 80)"
  surface-unexposed: "oklch(0.28 0.008 55)"
  text-primary: "oklch(0.955 0.008 85)"
  text-secondary: "oklch(0.79 0.012 75)"
  text-tertiary: "oklch(0.7 0.016 65)"
  text-on-accent: "oklch(0.18 0.02 50)"
  text-accent: "oklch(0.76 0.14 55)"
  border-subtle: "oklch(0.36 0.012 55)"
  border-control: "oklch(0.54 0.014 55)"
  accent: "oklch(0.72 0.155 52)"
  accent-hover: "oklch(0.76 0.14 55)"
  accent-soft: "oklch(0.72 0.155 52 / 0.14)"
  status-danger: "oklch(0.7 0.17 28)"
  focus-ring: "oklch(0.76 0.14 55)"
  selection: "oklch(0.72 0.155 52 / 0.35)"
  # Light theme (photographic paper), same semantic names under [data-theme='light']
  surface-base-light: "oklch(0.958 0.003 250)"
  surface-sunken-light: "oklch(0.925 0.005 250)"
  surface-print-light: "oklch(0.995 0.003 85)"
  surface-unexposed-light: "oklch(0.86 0.006 80)"
  text-primary-light: "oklch(0.23 0.015 50)"
  text-secondary-light: "oklch(0.42 0.016 55)"
  text-tertiary-light: "oklch(0.49 0.018 55)"
  text-on-accent-light: "oklch(0.995 0.003 85)"
  text-accent-light: "oklch(0.51 0.15 42)"
  border-subtle-light: "oklch(0.86 0.006 250)"
  border-control-light: "oklch(0.62 0.008 250)"
  accent-light: "oklch(0.56 0.155 44)"
  accent-hover-light: "oklch(0.52 0.15 42)"
  accent-soft-light: "oklch(0.56 0.155 44 / 0.1)"
  status-danger-light: "oklch(0.52 0.18 28)"
  focus-ring-light: "oklch(0.51 0.15 42)"
  selection-light: "oklch(0.56 0.155 44 / 0.22)"
typography:
  display:
    fontFamily: "Archivo, 'Segoe UI Variable Display', 'Segoe UI', system-ui, sans-serif"
    fontSize: "2rem"
    fontWeight: 700
    lineHeight: 1.15
    letterSpacing: "-0.005em"
    fontVariation: "'wdth' 87.5"
  headline:
    fontFamily: "Archivo, 'Segoe UI Variable Display', 'Segoe UI', system-ui, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 650
    lineHeight: 1.45
    fontVariation: "'wdth' 87.5"
  title:
    fontFamily: "Archivo, 'Segoe UI Variable Display', 'Segoe UI', system-ui, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 650
    lineHeight: 1.45
    fontVariation: "'wdth' 87.5"
  ratio-value:
    fontFamily: "Archivo, 'Segoe UI Variable Display', 'Segoe UI', system-ui, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 700
    lineHeight: 1
    fontFeature: "'tnum'"
    fontVariation: "'wdth' 75"
  body:
    fontFamily: "'Segoe UI Variable Text', 'Segoe UI', system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.45
  body-strong:
    fontFamily: "'Segoe UI Variable Text', 'Segoe UI', system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 600
    lineHeight: 1.45
  caption:
    fontFamily: "'Segoe UI Variable Text', 'Segoe UI', system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "Archivo, 'Segoe UI Variable Display', 'Segoe UI', system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 600
    lineHeight: "1.5rem"
    letterSpacing: "0.06em"
    fontFeature: "'tnum'"
    fontVariation: "'wdth' 75"
  wordmark:
    fontFamily: "Archivo, 'Segoe UI Variable Display', 'Segoe UI', system-ui, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 800
    letterSpacing: "0.06em"
    fontVariation: "'wdth' 75"
rounded:
  print: "1px"
  control: "4px"
spacing:
  space-1: "4px"
  space-2: "8px"
  space-3: "12px"
  space-4: "16px"
  space-5: "24px"
  space-6: "32px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.text-on-accent}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.control}"
    padding: "0 16px"
    height: "2.25rem"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.text-primary}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.control}"
    padding: "0 16px"
    height: "2.25rem"
  button-secondary-hover:
    textColor: "{colors.text-accent}"
  button-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.text-secondary}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.control}"
    padding: "0 8px"
    height: "2.25rem"
  button-quiet-hover:
    textColor: "{colors.text-primary}"
  ratio-frame:
    backgroundColor: "transparent"
    textColor: "{colors.text-primary}"
    typography: "{typography.ratio-value}"
    rounded: "{rounded.print}"
    padding: "8px 12px"
    height: "8rem"
  ratio-frame-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.text-on-accent}"
    typography: "{typography.ratio-value}"
    rounded: "{rounded.print}"
    padding: "8px 12px"
    height: "8rem"
  ratio-frame-primary-hover:
    backgroundColor: "{colors.accent-hover}"
  print-paper:
    backgroundColor: "{colors.surface-print}"
    rounded: "{rounded.print}"
    padding: "3px"
    height: "7.5rem"
  print-paper-hover:
    backgroundColor: "{colors.text-primary}"
  edge-print:
    backgroundColor: "{colors.surface-base}"
    textColor: "{colors.text-tertiary}"
    typography: "{typography.label}"
    padding: "4px 12px"
    height: "1.75rem"
  select:
    backgroundColor: "{colors.surface-base}"
    textColor: "{colors.text-primary}"
    typography: "{typography.caption}"
    rounded: "{rounded.control}"
    padding: "0 8px"
    height: "2rem"
  contact-sheet:
    backgroundColor: "{colors.surface-sunken}"
    padding: "32px"
---

# Design System: Kinora

<!-- Recorded from the shipped start screen (sprint A, UI foundation, 22 Oct 2026). Source of truth for values:
     src/lib/theme/tokens.css. Repo-wide UI architecture rules: docs/07-frontend-guidelines.md (binding). -->

## Overview

**Creative North Star: "Amplop Cuci-Cetak" (the photo-lab print envelope)**

Kinora's UI is the neighbourhood photo lab. Work comes back to the creator the way prints came back from the *cuci cetak* counter: a contact sheet read at a glance, each print standing on its own strip of edge-print numbers. The dark theme is the darkroom (warm film-rebate charcoal, print-white type); the light theme is bright photographic paper with the lab's warm ink. One accent, the orange of colour-negative base, marks the action; darkroom safelight red marks a problem.

Surfaces are flat and matte. Hierarchy comes from space, type voice and order-form hairlines, never from boxes, gradients or glow. Two type voices share the work: the Windows system face carries every sentence, and condensed Archivo carries the lab's printed voice (headings, the wordmark, and the edge-print caps with tabular numbers). Density is calm: one column to start something, one sunken field holding past work.

The system was built from a refusal of the category default: a grey dashboard of identical project cards with a blue accent. The PRODUCT.md commitment "no generic purple gradients, identical cards or decorative icons" holds across every surface.

**Key Characteristics:**
- Warm charcoal darkroom and near-neutral photo paper, both themes first-class and AA-checked.
- One accent (negative-base orange) for the primary action, selection, hover and focus.
- Edge-print strips: condensed caps, open tracking, tabular numbers, duration set against the right edge.
- Prints in thin paper borders, one height, one baseline, whatever their ratio.
- Hairline ruling instead of boxes; 1px print corners, 4px control corners.

## Colors

A warm, low-chroma ground (hue 55) with a single saturated orange; status colours borrowed from darkroom safelights. All values are OKLCH; components use only the semantic names below, never the `--pigment-*` primitives (docs/07 §7.1).

### Primary
- **Negative-Base Orange** (`accent`): the default ratio frame (9:16) is filled with it; the primary button; the hover border of every ratio frame and secondary button; the drop band's active rule and the strip's frame numbers (via `text-accent`). In light theme it deepens (`accent-light`) so white text on it stays AA.
- **Bright Negative** (`accent-hover`, also `text-accent` and `focus-ring` in dark): hover fill of accent elements, accent-coloured text, and the 2px focus outline.
- **Negative Wash** (`accent-soft`): the only translucent fill, used solely for the drop band while a file is dragged over the window.
- **Selection** (`selection`): text selection highlight, accent at 35% (dark) or 22% (light).

### Neutral
- **Film-Rebate Charcoal** (`surface-base`): window ground, the start column, and the edge-print strip background.
- **Rebate Deep** (`surface-sunken`): the contact-sheet field, one step darker in dark theme (one step shaded in light) so past work reads as a separate tray without a border.
- **Print Border** (`surface-print`): the paper border around each print. In dark it is deliberately dimmed below print white so prints never outshine the primary action; in light it is the brightest paper.
- **Unexposed Frame** (`surface-unexposed`): the image area of a print until thumbnails exist.
- **Print White / Lab Ink** (`text-primary`): headings, project names, body copy; also the print border on hover and focus.
- **Secondary** (`text-secondary`) for hints and durations; **Tertiary** (`text-tertiary`) for edge-print labels, counts, shortcut keys and settings labels.
- **Rule** (`border-subtle`) for hairlines and quiet field strokes; **Control Stroke** (`border-control`) for secondary buttons, unselected ratio frames and scrollbars.

### Status
- **Safelight Red** (`status-danger`): the edge-print status of a project whose folder is missing. Always paired with text, never colour alone.

`--status-working` (safelight amber), `--status-done`, `--surface-raised` and `--shadow-lift` are defined in tokens.css but no shipped surface uses them yet; they are reserved, not established.

### Named Rules
**The One Accent Rule.** Orange is the only hue that signals intent: the primary action, selection, hover and focus. No second accent; status colours never stand in for decoration.

**The Semantic Layer Rule.** Components read `--surface-*`, `--text-*`, `--border-*`, `--accent*`, `--status-*`, `--focus-ring`. Raw `--pigment-*` values and literal colours stay inside tokens.css.

## Typography

**Display Font:** Archivo variable (bundled, OFL, `wdth` 62–125) with Segoe UI Variable Display and system fallbacks
**Body Font:** Segoe UI Variable Text (with Segoe UI, system-ui)
**Label Font:** Archivo at 75% width, uppercase, tracked

**Character:** The system face keeps sentences native to Windows and ClearType-sharp; Archivo, condensed, is the lab's printed voice: order-form headings at 87.5% width and edge-print caps at 75%.

### Hierarchy
- **Display** (700, 2rem / 32px, 1.15, 87.5% width): the screen's one heading ("Proyek baru").
- **Headline** (650, 1.125rem / 18px, 87.5% width): section headings ("Terakhir dibuka") and the empty-state caption on the unexposed strip (that caption at 600).
- **Title** (650, 0.9375rem / 15px, 87.5% width): band headings such as "Seret video ke sini".
- **Ratio value** (700, 1.5rem / 24px, line-height 1, 75% width, tabular): the ratio written inside its frame.
- **Body** (400, 0.8125rem / 13px, 1.45): all sentences; 13px is the body minimum. **Body strong** (600) for button labels and project names.
- **Caption** (400, 0.75rem / 12px): settings labels and secondary path lines.
- **Label** (600, 0.75rem / 12px, 0.06em tracking, uppercase, 75% width, tabular): edge prints, ratio captions, shortcut keys.
- **Wordmark** (800, 1.125rem, 0.06em, uppercase, 75% width): "KINORA" in the top bar.

### Named Rules
**The Two Voices Rule.** Archivo never sets a sentence; the system face never sets an edge print. Paragraphs, hints and errors are always the system UI font.

**The Tabular Rule.** Every number that can change or be compared (frame numbers, durations, ratios, timecodes) uses tabular figures.

## Layout

The window is a top bar over a two-column body. The bar (12px / 32px padding) holds the wordmark left and quick settings right, ruled off by a hairline. The start column is `minmax(22rem, 38%)`; the contact sheet takes the rest as a sunken field. Each column scrolls on its own; the window does not.

Spacing is a 4px-based scale (`space-1`…`space-6`, 4–32px). Columns pad at 32px; stacked groups in the start column sit 24px apart; tight pairs (heading and hint) 8px apart.

Sizing is driven by container queries, not breakpoints. The three ratio frames share one height computed from the column width (`min(10rem, (100cqi − 3rem) / 3.34)`), so 9:16, 16:9 and 1:1 always fit side by side. The contact sheet is an auto-fill grid of 14.5rem minimum cells with 24px row gaps and **no column gap**, so each row's edge prints join into one continuous rebate band. The 1366×768 viewport is the comfort floor; wider windows get more prints per row, not a different layout.

### Named Rules
**The One Baseline Rule.** Frames and prints in a row share one height (ratio frames by `--ratio-height`, prints 7.5rem) and stand bottom-aligned on one line; width follows each item's ratio.

## Elevation & Depth

Flat. No shipped surface casts a shadow. Depth is tonal: the contact sheet sits one lightness step below the base (`surface-sunken`), and the edge-print strips sit back on `surface-base` within it. Separation otherwise comes from 1px hairlines (`border-subtle`).

A `--shadow-lift` token exists but is unused; it is not sanctioned for resting surfaces and should only be adopted, deliberately, by a genuinely floating layer (menu, popover) when one is built.

### Named Rules
**The Matte Rule.** No gradients, no glow, no resting shadows. A surface is distinguished by its tone or a hairline, never by light.

## Shapes

Two corner radii and nothing in between: prints and frames are cut paper (`rounded.print`, 1px), interactive controls are softly squared (`rounded.control`, 4px). No pills, no circles. Strokes are 1px hairlines (`--hairline`), with two heavier exceptions that belong to the world: the 3px paper border of a print (`--print-border`) and the 2px stroke of a ratio frame. Rules run edge to edge as order-form lines (`border-block`) rather than enclosing boxes.

## Components

### Buttons
Quiet, squared, decisive. One component, three variants (`Button.svelte`).
- **Shape:** softly squared (4px), min height 2.25rem, 16px inline padding, 8px gap between icon, label and shortcut.
- **Primary:** negative-orange fill with `text-on-accent`; hover moves to the brighter negative. One per screen at most.
- **Secondary:** transparent with a `border-control` hairline and `text-primary`; hover turns border to accent and label to `text-accent`. The default variant ("Buka proyek…", "Pilih video…", recovery actions).
- **Quiet:** transparent, `text-secondary`, 8px padding; hover to `text-primary`. For dense areas.
- **Shortcut key:** an optional `kbd` after the label in label voice, `text-tertiary`.
- **Icon:** optional, 16px Lucide line icon at stroke 1.75, always beside a visible label.
- **Focus / Disabled:** global 2px `focus-ring` outline at 2px offset; disabled at 50% opacity with a not-allowed cursor.
- **Transition:** background, border and text colour over `--duration-fast` (120ms) on `--ease-out`.

### Ratio Frames (signature)
The new-project choice drawn as the shape it creates.
- A 2px `border-control` frame at 1px corners, aspect-locked (9:16, 16:9, 1:1), the ratio set inside bottom-left in ratio-value type.
- The default (9:16) is filled with accent and is the screen's primary action (Ctrl+N); others take an accent border on hover.
- Caption under the frame in label voice: name (`text-accent` for the default), use case and shortcut in `text-tertiary`.

### Drop Band
An order-form band, ruled above and below, no box. Title and hint on the left, a secondary "choose video" button on the right. While a file is dragged anywhere over the window, the rules turn accent, the band fills with `accent-soft`, gains 12px inline padding and the title switches to `text-accent` (border 120ms, background 200ms).

### Contact Sheet and Prints (signature)
- **Sheet:** the sunken field (`surface-sunken`, 32px padding), headline plus a tertiary project count.
- **Print:** paper border (`surface-print`, 3px, 1px corners) around the unexposed image, sized from the project's ratio inside a fixed 7.5rem stage. The whole print is one button: hover and focus turn the paper to `text-primary`, hover turns the name to `text-accent`.
- **Missing project:** paper at 40% opacity, name in `text-secondary`, last location in caption type, edge print showing the status in `status-danger`, and two secondary buttons (locate, forget) in place of opening.
- **States:** loading as a secondary-text note; error as text, detail and a secondary retry button; first run as the unexposed strip below.

### Edge Print (signature)
The strip under every frame and Kinora's reusable signature (intended to become the timeline ruler): two-digit frame number in `text-accent`, ratio, date (ellipsised), and duration pushed to the right edge in `text-secondary`. Label voice throughout on `surface-base`, 1.75rem min height, 4px / 12px padding.

### Unexposed Strip (empty state)
Four blank 9:16 frames drawn as `border-subtle` hairlines at 1px corners over numbered edge prints, captioned underneath with the first step in headline voice. The empty state always says what to do first.

### Inputs / Fields (quick settings)
Native selects for language and appearance: `border-subtle` hairline, 4px corners, `surface-base` fill, 2rem height, caption type; hover strengthens the stroke to `border-control`. Labels sit inline in caption type, `text-tertiary`.

### Open Items (contract ceilings not yet built)
- **Print-envelope motif:** the "amplop" itself has no visual expression yet; only the contact sheet and edge print are built.
- **One designed motion moment:** `--duration-develop` (360ms) is reserved for it and unused; no develop/reveal motion ships.
- **Light-theme hue:** paper and rules run cool (hue 250) against warm ink (hue 50–55) and orange; the light world's temperature is still to be settled.
- **Playhead and selection in the editor, amber "working" status, real thumbnails** (needs the shell-side thumbnail cache): not built.

## Do's and Don'ts

### Do:
- **Do** use only semantic tokens from tokens.css in components, and check every new pair for WCAG AA in both themes.
- **Do** keep one accent: negative-base orange for the primary action, selection, hover and focus.
- **Do** set edge prints, ratio captions and shortcut keys in Archivo at 75% width, 600, uppercase, 0.06em tracking, tabular figures.
- **Do** keep sentences in the system UI face at 13px minimum.
- **Do** stand frames and prints on one baseline at one height, with their width following the ratio.
- **Do** separate fields with 1px hairlines and tonal steps (`surface-sunken`) rather than boxes.
- **Do** pair every status colour with text, and give every icon a visible label.
- **Do** use `--duration-fast`/`--duration-base` with `--ease-out`; the reduced-motion query already zeroes them.

### Don't:
- **Don't** add gradients, glow, or resting shadows.
- **Don't** introduce a second accent hue or a blue "app" accent.
- **Don't** tint print images or let a print border carry the accent colour.
- **Don't** round corners beyond 4px or use pill shapes.
- **Don't** set paragraphs or error messages in Archivo.
- **Don't** line up identical cards; past work is a contact sheet of prints, not a card grid.
