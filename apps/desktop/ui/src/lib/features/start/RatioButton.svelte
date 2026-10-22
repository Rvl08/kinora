<!--
  One ratio choice for a new project. The frame is the shape it creates:
  all three share one height and stand on one baseline, like the prints on
  the contact sheet. The default (9:16) is filled: it is the action. Labels
  sit under the frame in edge-print caps, so the frame stays uncluttered.
-->
<script lang="ts">
  import { i18n } from '$lib/i18n/index.svelte';

  import { presetLabel, type RatioPreset } from './ratio';

  interface Props {
    preset: RatioPreset;
    /** The default choice: filled with the accent. */
    primary?: boolean;
    /** The key after Ctrl that triggers it, shown under the frame. */
    shortcut?: string | undefined;
    /** Show the use case under the frame (narrow frames get a hint line instead). */
    showUse?: boolean;
    onselect: (preset: RatioPreset) => void;
  }

  let { preset, primary = false, shortcut, showUse = true, onselect }: Props = $props();

  const label = $derived(presetLabel(preset));
</script>

<button
  type="button"
  class="ratio"
  class:primary
  data-preset={preset}
  aria-keyshortcuts={shortcut ? `Control+${shortcut.toUpperCase()}` : undefined}
  onclick={() => onselect(preset)}
>
  <span class="frame"><span class="value">{label}</span></span>
  <span class="caption">
    <span class="name">{i18n.t(`start.ratio.${preset}`)}</span>
    <span class={showUse ? 'use' : 'visually-hidden'}>{i18n.t(`start.ratio.${preset}Use`)}</span>
    {#if shortcut}
      <kbd class="key">{i18n.t('keys.ctrl')}+{shortcut.toUpperCase()}</kbd>
    {/if}
  </span>
</button>

<style>
  .ratio {
    display: grid;
    gap: var(--space-2);
    justify-items: start;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-primary);
    text-align: left;
  }

  .frame {
    display: flex;
    align-items: flex-end;
    height: var(--ratio-height, 8rem);
    padding: var(--space-2) var(--space-3);
    border: 2px solid var(--border-control);
    border-radius: var(--radius-print);
    transition:
      border-color var(--duration-fast) var(--ease-out),
      background-color var(--duration-fast) var(--ease-out);
  }

  [data-preset='vertical'] .frame {
    aspect-ratio: 9 / 16;
  }

  [data-preset='landscape'] .frame {
    aspect-ratio: 16 / 9;
  }

  [data-preset='square'] .frame {
    aspect-ratio: 1;
  }

  .ratio:hover .frame {
    border-color: var(--accent);
  }

  .primary .frame {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--text-on-accent);
  }

  .primary:hover .frame {
    border-color: var(--accent-hover);
    background: var(--accent-hover);
  }

  .value {
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 700;
    font-size: var(--text-xl);
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }

  .caption {
    display: grid;
    gap: 0.125rem;
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 600;
    font-size: var(--text-xs);
    letter-spacing: var(--edge-tracking);
    text-transform: uppercase;
    white-space: nowrap;
  }

  .name {
    color: var(--text-primary);
  }

  .primary .name {
    color: var(--text-accent);
  }

  .use,
  .key {
    color: var(--text-tertiary);
  }
</style>
