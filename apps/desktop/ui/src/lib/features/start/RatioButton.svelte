<!--
  One ratio choice for a new project, drawn as the frame it will produce.
-->
<script lang="ts">
  import { i18n } from '$lib/i18n/index.svelte';

  import { presetLabel, type RatioPreset } from './ratio';

  interface Props {
    preset: RatioPreset;
    /** The default choice gets the accent. */
    primary?: boolean;
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
  aria-keyshortcuts={shortcut}
  onclick={() => onselect(preset)}
>
  <span class="stage" aria-hidden="true"><span class="frame"></span></span>
  <span class="value">{label}</span>
  <span class="name">{i18n.t(`start.ratio.${preset}`)}</span>
  <span class="use">{i18n.t(`start.ratio.${preset}Use`)}</span>
</button>

<style>
  .ratio {
    display: grid;
    align-content: start;
    justify-items: start;
    gap: var(--space-1);
    padding: var(--space-3);
    border: var(--hairline) solid var(--border-subtle);
    border-radius: var(--radius-control);
    background: var(--surface-raised);
    text-align: left;
    transition:
      border-color var(--duration-fast) var(--ease-out),
      box-shadow var(--duration-base) var(--ease-out);
  }

  .ratio:hover {
    border-color: var(--border-control);
    box-shadow: var(--shadow-lift);
  }

  .stage {
    display: grid;
    place-items: end start;
    width: 100%;
    height: 4.5rem;
    margin-bottom: var(--space-2);
  }

  .frame {
    display: block;
    height: 100%;
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
    height: auto;
    width: 100%;
    max-width: 8rem;
    aspect-ratio: 16 / 9;
  }

  [data-preset='square'] .frame {
    height: 3.5rem;
    aspect-ratio: 1;
  }

  .ratio:hover .frame {
    border-color: var(--accent);
  }

  .primary {
    border-color: var(--accent);
  }

  .primary .frame {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .value {
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 700;
    font-size: var(--text-xl);
    line-height: 1;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.01em;
  }

  .primary .value {
    color: var(--text-accent);
  }

  .name {
    font-weight: 600;
  }

  .use {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }
</style>
