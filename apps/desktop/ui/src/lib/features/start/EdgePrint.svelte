<!--
  The edge print: the strip of tabular labels a contact sheet carries under
  each frame, and Kinora's signature (the timeline ruler reuses it): frame
  number, ratio, date, and the duration set tight against the right edge.
-->
<script lang="ts">
  interface Props {
    frame: number;
    ratio?: string;
    date?: string;
    duration?: string;
    status?: string | undefined;
  }

  let { frame, ratio = '', date = '', duration = '', status }: Props = $props();
</script>

<p class="edge">
  <span class="number">{String(frame).padStart(2, '0')}</span>
  {#if status}
    <span class="status">{status}</span>
  {:else}
    <span>{ratio}</span>
    <span class="date">{date}</span>
  {/if}
  <span class="duration">{duration}</span>
</p>

<style>
  .edge {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    min-height: 1.75rem;
    padding: var(--space-1) var(--space-3);
    background: var(--surface-base);
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 600;
    font-size: var(--text-xs);
    line-height: 1.5rem;
    letter-spacing: var(--edge-tracking);
    text-transform: uppercase;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    color: var(--text-tertiary);
  }

  .number {
    color: var(--text-accent);
  }

  .date {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .duration {
    margin-left: auto;
    color: var(--text-secondary);
  }

  .status {
    color: var(--status-danger);
  }
</style>
