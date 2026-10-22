<!--
  The start screen: start a project (a ratio, or a dropped video), open one,
  or pick up a recent one from the contact sheet.
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
    ondropmedia: (files: readonly File[]) => void;
    onchoosevideo: () => void;
  }

  let { recent, now, oncreate, onopen, onopenrecent, ondropmedia, onchoosevideo }: Props = $props();

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
</script>

<svelte:window onkeydown={shortcuts} />

<div class="start">
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
            shortcut={index === 0 ? 'Control+N' : undefined}
            onselect={oncreate}
          />
        {/each}
      </div>

      <DropBand ondrop={ondropmedia} onchoose={onchoosevideo} />

      <div class="open">
        <Button variant="secondary" aria-keyshortcuts="Control+O" onclick={onopen}>
          {#snippet icon()}<FolderOpen size={16} strokeWidth={1.75} />{/snippet}
          {i18n.t('start.openProject')}
        </Button>
      </div>
    </section>

    <ContactSheet {recent} {now} onopen={onopenrecent} />
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
    font-size: var(--text-2xl);
    font-weight: 700;
    line-height: var(--leading-tight);
    letter-spacing: -0.01em;
  }

  .intro p {
    color: var(--text-secondary);
  }

  .ratios {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-3);
  }

  .open {
    display: flex;
  }
</style>
