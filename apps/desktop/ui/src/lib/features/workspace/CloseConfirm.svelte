<!--
  Closing with unsaved changes loses them, so it is the one place that asks
  first (docs/07 §8). A modal <dialog>: focus is trapped, Escape cancels, and
  saving is the first, default choice.
-->
<script lang="ts">
  import { onMount } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import { i18n } from '$lib/i18n/index.svelte';

  interface Props {
    busy: boolean;
    onsave: () => void;
    ondiscard: () => void;
    oncancel: () => void;
  }

  let { busy, onsave, ondiscard, oncancel }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();

  onMount(() => {
    // The dialog leaves the page instead of closing in place, so give focus
    // back to whatever opened it ("Close project") ourselves.
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog?.showModal();
    return () => {
      dialog?.close();
      if (opener?.isConnected) opener.focus();
    };
  });

  function cancel(event: Event) {
    event.preventDefault();
    if (!busy) oncancel();
  }
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="close-title"
  aria-describedby="close-body"
  oncancel={cancel}
>
  <h2 id="close-title">{i18n.t('workspace.confirmClose.title')}</h2>
  <p id="close-body">{i18n.t('workspace.confirmClose.body')}</p>
  <div class="actions">
    <Button variant="primary" disabled={busy} onclick={onsave}>
      {i18n.t('workspace.confirmClose.save')}
    </Button>
    <Button variant="secondary" disabled={busy} onclick={ondiscard}>
      {i18n.t('workspace.confirmClose.discard')}
    </Button>
    <Button variant="quiet" disabled={busy} onclick={oncancel}>
      {i18n.t('workspace.confirmClose.cancel')}
    </Button>
  </div>
</dialog>

<style>
  dialog {
    display: grid;
    gap: var(--space-3);
    width: min(28rem, calc(100vw - 2 * var(--space-6)));
    padding: var(--space-5);
    border: var(--hairline) solid var(--border-subtle);
    border-radius: var(--radius-control);
    background: var(--surface-raised);
    color: var(--text-primary);
    box-shadow: var(--shadow-lift);
  }

  dialog:not([open]) {
    display: none;
  }

  dialog::backdrop {
    background: var(--scrim);
  }

  h2 {
    font-family: var(--font-display);
    font-stretch: 87.5%;
    font-weight: 650;
    font-size: var(--text-lg);
  }

  p {
    color: var(--text-secondary);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
</style>
