<!--
  Drop a video to start a vertical project from it. Pointer users drop
  files; everyone can use the button, which opens the file dialog.
-->
<script lang="ts">
  import FileVideo from '@lucide/svelte/icons/file-video';

  import Button from '$lib/components/Button.svelte';
  import { i18n } from '$lib/i18n/index.svelte';

  interface Props {
    ondrop: (files: readonly File[]) => void;
    onchoose: () => void;
  }

  let { ondrop, onchoose }: Props = $props();

  let active = $state(false);
  // dragenter/dragleave fire for every child; count them.
  let depth = 0;

  const hasFiles = (event: DragEvent) => event.dataTransfer?.types.includes('Files') ?? false;

  function enter(event: DragEvent) {
    if (!hasFiles(event)) return;
    event.preventDefault();
    depth += 1;
    active = true;
  }

  function over(event: DragEvent) {
    if (!hasFiles(event)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
  }

  function leave() {
    depth = Math.max(0, depth - 1);
    if (depth === 0) active = false;
  }

  function drop(event: DragEvent) {
    event.preventDefault();
    depth = 0;
    active = false;
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (files.length > 0) ondrop(files);
  }
</script>

<section
  class="band"
  class:active
  aria-labelledby="drop-title"
  ondragenter={enter}
  ondragover={over}
  ondragleave={leave}
  ondrop={drop}
>
  <span class="glyph" aria-hidden="true"><FileVideo size={28} strokeWidth={1.5} /></span>
  <div class="copy">
    <h2 id="drop-title">{active ? i18n.t('start.dropActive') : i18n.t('start.dropTitle')}</h2>
    <p>{i18n.t('start.dropHint')}</p>
  </div>
  <Button variant="secondary" onclick={onchoose}>{i18n.t('start.chooseVideo')}</Button>
</section>

<style>
  .band {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    padding: var(--space-5);
    background: var(--surface-sunken);
    border: var(--hairline) solid var(--border-subtle);
    border-radius: var(--radius-control);
    transition:
      border-color var(--duration-fast) var(--ease-out),
      background-color var(--duration-base) var(--ease-out);
  }

  .band :global(.button) {
    grid-column: 2;
    justify-self: start;
  }

  .active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .glyph {
    display: inline-flex;
    color: var(--text-tertiary);
    align-self: start;
  }

  .active .glyph {
    color: var(--text-accent);
  }

  h2 {
    font-size: var(--text-md);
    font-weight: 600;
  }

  p {
    color: var(--text-secondary);
  }
</style>
