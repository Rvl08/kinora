<!--
  The app's button. `primary` is the one main action of a screen, `secondary`
  the ordinary one, `quiet` for low-emphasis actions in dense areas.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  interface Props extends Omit<HTMLButtonAttributes, 'children'> {
    variant?: 'primary' | 'secondary' | 'quiet';
    icon?: Snippet;
    children: Snippet;
  }

  let { variant = 'secondary', icon, children, type = 'button', ...rest }: Props = $props();
</script>

<button {type} class="button {variant}" {...rest}>
  {#if icon}<span class="icon" aria-hidden="true">{@render icon()}</span>{/if}
  <span>{@render children()}</span>
</button>

<style>
  .button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 2.25rem;
    padding: 0 var(--space-4);
    border: var(--hairline) solid transparent;
    border-radius: var(--radius-control);
    font-weight: 600;
    font-size: var(--text-sm);
    white-space: nowrap;
    transition:
      background-color var(--duration-fast) var(--ease-out),
      border-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .primary {
    background: var(--accent);
    color: var(--text-on-accent);
  }

  .primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .secondary {
    background: transparent;
    border-color: var(--border-control);
    color: var(--text-primary);
  }

  .secondary:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--text-accent);
  }

  .quiet {
    background: transparent;
    color: var(--text-secondary);
    padding: 0 var(--space-2);
  }

  .quiet:hover:not(:disabled) {
    color: var(--text-primary);
  }

  .button:disabled {
    opacity: 0.5;
  }

  .icon {
    display: inline-flex;
  }
</style>
