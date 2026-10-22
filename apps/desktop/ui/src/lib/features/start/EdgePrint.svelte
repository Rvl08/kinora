<!--
  The edge print: the strip of tabular labels a contact sheet carries under
  each frame. Kinora's signature: frame number, ratio, and the duration set
  tight against the right edge. The timeline ruler reuses it later.
-->
<script lang="ts">
  interface Props {
    frame: number;
    ratio: string;
    duration: string;
    status?: string | undefined;
  }

  let { frame, ratio, duration, status }: Props = $props();
</script>

<p class="edge">
  <span class="number">{String(frame).padStart(2, '0')}</span>
  {#if status}
    <span class="status">{status}</span>
  {:else}
    <span>{ratio}</span>
    <span class="duration">{duration}</span>
  {/if}
</p>

<style>
  .edge {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    padding-top: var(--space-2);
    border-top: var(--hairline) solid var(--border-subtle);
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 600;
    font-size: var(--text-xs);
    letter-spacing: var(--edge-tracking);
    text-transform: uppercase;
    font-variant-numeric: tabular-nums;
    color: var(--text-tertiary);
  }

  .number {
    color: var(--text-accent);
  }

  .duration {
    margin-left: auto;
    color: var(--text-secondary);
  }

  .status {
    color: var(--status-danger);
  }
</style>
