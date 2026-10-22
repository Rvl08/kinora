<!--
  The band that starts a vertical project from a video. The whole window
  accepts the drop (StartScreen tracks it and passes `active`); the button
  opens the file dialog for keyboard and touch users.
-->
<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { i18n } from '$lib/i18n/index.svelte';

  interface Props {
    /** A file is being dragged over the window. */
    active: boolean;
    onchoose: () => void;
  }

  let { active, onchoose }: Props = $props();
</script>

<section class="band" class:active aria-labelledby="drop-title">
  <div class="copy">
    <h2 id="drop-title">{active ? i18n.t('start.dropActive') : i18n.t('start.dropTitle')}</h2>
    <p>{i18n.t('start.dropHint')}</p>
  </div>
  <Button variant="secondary" onclick={onchoose}>{i18n.t('start.chooseVideo')}</Button>
</section>

<style>
  /* An order-form band: ruled above and below, no box. */
  .band {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    padding: var(--space-4) 0;
    border-block: var(--hairline) solid var(--border-subtle);
    transition:
      border-color var(--duration-fast) var(--ease-out),
      background-color var(--duration-base) var(--ease-out);
  }

  .active {
    border-color: var(--accent);
    background: var(--accent-soft);
    padding-inline: var(--space-3);
  }

  h2 {
    font-family: var(--font-display);
    font-stretch: 87.5%;
    font-weight: 650;
    font-size: var(--text-md);
  }

  .active h2 {
    color: var(--text-accent);
  }

  p {
    color: var(--text-secondary);
  }
</style>
