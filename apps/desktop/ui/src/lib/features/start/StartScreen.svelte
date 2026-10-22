<!--
  The start screen: start a project (a ratio, or a video dropped anywhere on
  the window), open one, or pick up a recent one from the contact sheet.
-->
<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';

  import Button from '$lib/components/Button.svelte';
  import QuickSettings from '$lib/features/settings/QuickSettings.svelte';
  import type { RecentProject } from '$lib/ipc/bindings';
  import { i18n } from '$lib/i18n/index.svelte';
  import type { RecentProjectsStore } from '$lib/state/recent.svelte';

  import ContactSheet from './ContactSheet.svelte';
  import DropBand from './DropBand.svelte';
  import RatioButton from './RatioButton.svelte';
  import { RATIO_PRESETS, type RatioPreset } from './ratio';

  interface Props {
    recent: RecentProjectsStore;
    now: number;
    oncreate: (preset: RatioPreset) => void;
    onopen: () => void;
    onopenrecent: (project: RecentProject) => void;
    onlocaterecent: (project: RecentProject) => void;
    onforgetrecent: (project: RecentProject) => void;
    ondropmedia: (files: readonly File[]) => void;
    onchoosevideo: () => void;
  }

  let {
    recent,
    now,
    oncreate,
    onopen,
    onopenrecent,
    onlocaterecent,
    onforgetrecent,
    ondropmedia,
    onchoosevideo,
  }: Props = $props();

  // Ctrl+N starts the default (vertical) project, Ctrl+O opens one.
  function shortcuts(event: KeyboardEvent) {
    if (!event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return;
    if (event.key === 'n') {
      event.preventDefault();
      oncreate('vertical');
    } else if (event.key === 'o') {
      event.preventDefault();
      onopen();
    }
  }

  // The whole window accepts a dropped video. dragenter/dragleave fire for
  // every child element, so count them.
  let dragging = $state(false);
  let depth = 0;

  const hasFiles = (event: DragEvent) => event.dataTransfer?.types.includes('Files') ?? false;

  function dragenter(event: DragEvent) {
    if (!hasFiles(event)) return;
    event.preventDefault();
    depth += 1;
    dragging = true;
  }

  function dragover(event: DragEvent) {
    if (!hasFiles(event)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
  }

  function dragleave() {
    depth = Math.max(0, depth - 1);
    if (depth === 0) dragging = false;
  }

  function drop(event: DragEvent) {
    event.preventDefault();
    depth = 0;
    dragging = false;
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (files.length > 0) ondropmedia(files);
  }
</script>

<svelte:window onkeydown={shortcuts} />

<div
  class="start"
  role="presentation"
  ondragenter={dragenter}
  ondragover={dragover}
  ondragleave={dragleave}
  ondrop={drop}
>
  <header class="bar">
    <p class="mark">Kinora</p>
    <QuickSettings />
  </header>

  <main class="layout">
    <section class="begin" aria-labelledby="new-title">
      <div class="intro">
        <h1 id="new-title">{i18n.t('start.newProject')}</h1>
        <p>{i18n.t('start.newProjectHint')}</p>
      </div>

      <div class="ratios">
        {#each RATIO_PRESETS as preset, index (preset)}
          <RatioButton
            {preset}
            primary={index === 0}
            shortcut={index === 0 ? 'n' : undefined}
            showUse={index !== 0}
            onselect={oncreate}
          />
        {/each}
      </div>
      <p class="hint">{i18n.t('start.verticalHint')}</p>

      <DropBand active={dragging} onchoose={onchoosevideo} />

      <div class="open">
        <Button
          variant="secondary"
          aria-keyshortcuts="Control+O"
          shortcut={`${i18n.t('keys.ctrl')}+O`}
          onclick={onopen}
        >
          {#snippet icon()}<FolderOpen size={16} strokeWidth={1.75} />{/snippet}
          {i18n.t('start.openProject')}
        </Button>
      </div>
    </section>

    <ContactSheet
      {recent}
      {now}
      onopen={onopenrecent}
      onlocate={onlocaterecent}
      onforget={onforgetrecent}
    />
  </main>
</div>

<style>
  .start {
    display: grid;
    grid-template-rows: auto 1fr;
    height: 100%;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-6);
    border-bottom: var(--hairline) solid var(--border-subtle);
  }

  .mark {
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 800;
    font-size: var(--text-lg);
    letter-spacing: var(--edge-tracking);
    text-transform: uppercase;
  }

  .layout {
    display: grid;
    grid-template-columns: minmax(22rem, 38%) 1fr;
    min-height: 0;
  }

  .begin {
    container-type: inline-size;
    display: grid;
    align-content: start;
    gap: var(--space-5);
    padding: var(--space-6);
    overflow-y: auto;
  }

  .intro {
    display: grid;
    gap: var(--space-2);
  }

  h1 {
    font-family: var(--font-display);
    font-stretch: 87.5%;
    font-size: var(--text-2xl);
    font-weight: 700;
    line-height: var(--leading-tight);
    letter-spacing: -0.005em;
  }

  .intro p {
    color: var(--text-secondary);
  }

  /* Frames share one height and stand on one baseline. The height fits
     the column: 9/16 + 16/9 + 1 = 3.34 heights of width, plus two gaps and
     some air on the right. */
  .ratios {
    --ratio-height: min(10rem, calc((100cqi - 3rem) / 3.34));
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }

  .hint {
    margin-top: calc(-1 * var(--space-3));
    color: var(--text-secondary);
  }

  .open {
    display: flex;
  }
</style>
