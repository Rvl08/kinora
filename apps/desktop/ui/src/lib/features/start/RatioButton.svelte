<!--
  One ratio choice for a new project. The button's own outline is the frame
  it creates: all three share one height and one baseline, like the prints
  on the contact sheet. The default (9:16) is filled: it is the action.
-->
<script lang="ts">
  import { i18n } from '$lib/i18n/index.svelte';

  import { presetLabel, type RatioPreset } from './ratio';

  interface Props {
    preset: RatioPreset;
    /** The default choice: filled with the accent. */
    primary?: boolean;
    /** The key after Ctrl that triggers it, shown on the frame. */
    shortcut?: string | undefined;
    onselect: (preset: RatioPreset) => void;
  }

  let { preset, primary = false, shortcut, onselect }: Props = $props();

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
  {#if shortcut}
    <kbd class="key">{i18n.t('keys.ctrl')}+{shortcut.toUpperCase()}</kbd>
  {/if}
  <span class="value">{label}</span>
  <span class="name">{i18n.t(`start.ratio.${preset}`)}</span>
  <span class="use">{i18n.t(`start.ratio.${preset}Use`)}</span>
</button>

<style>
  .ratio {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: flex-start;
    gap: 0.125rem;
    height: var(--ratio-height, 8rem);
    padding: var(--space-2);
    border: 2px solid var(--border-control);
    border-radius: var(--radius-print);
    background: transparent;
    color: var(--text-primary);
    text-align: left;
    transition:
      border-color var(--duration-fast) var(--ease-out),
      background-color var(--duration-fast) var(--ease-out);
  }

  [data-preset='vertical'] {
    aspect-ratio: 9 / 16;
  }

  [data-preset='landscape'] {
    aspect-ratio: 16 / 9;
  }

  [data-preset='square'] {
    aspect-ratio: 1;
  }

  .ratio:hover {
    border-color: var(--accent);
  }

  .primary {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--text-on-accent);
  }

  .primary:hover {
    border-color: var(--accent-hover);
    background: var(--accent-hover);
  }

  .key {
    margin-bottom: auto;
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 600;
    font-size: var(--text-xs);
    letter-spacing: var(--edge-tracking);
    text-transform: uppercase;
    opacity: 0.85;
  }

  .value {
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 700;
    font-size: var(--text-xl);
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }

  .name {
    font-weight: 600;
    font-size: var(--text-xs);
  }

  .use {
    font-size: var(--text-xs);
    line-height: 1.3;
    white-space: pre-line;
    color: var(--text-tertiary);
  }

  /* Full strength on the fill: the accent pair is only AA at full opacity. */
  .primary .use,
  .primary .key {
    color: inherit;
    opacity: 1;
  }
</style>
