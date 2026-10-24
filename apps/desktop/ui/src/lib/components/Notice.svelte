<!--
  A problem to report: what failed, why, and a way to dismiss it. An
  order-form band (ruled above and below, no box); the first line carries the
  safelight red, always as words. Place it inside an `aria-live` region.
-->
<script lang="ts">
  import Button from './Button.svelte';

  interface Props {
    /** What failed. */
    message: string;
    /** Why, and what to do next. */
    detail?: string | undefined;
    dismissLabel: string;
    ondismiss: () => void;
  }

  let { message, detail, dismissLabel, ondismiss }: Props = $props();
</script>

<div class="notice">
  <div class="copy">
    <p class="message">{message}</p>
    {#if detail}<p class="detail">{detail}</p>{/if}
  </div>
  <Button variant="quiet" onclick={ondismiss}>{dismissLabel}</Button>
</div>

<style>
  .notice {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
    padding: var(--space-3) 0;
    border-block: var(--hairline) solid var(--border-subtle);
  }

  .copy {
    display: grid;
    gap: var(--space-1);
    padding-top: var(--space-2);
  }

  .message {
    font-weight: 600;
    color: var(--status-danger);
  }

  .detail {
    color: var(--text-secondary);
  }
</style>
