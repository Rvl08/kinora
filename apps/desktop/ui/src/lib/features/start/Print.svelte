<!--
  One recent project on the contact sheet: the print in its own ratio, then
  the edge print, the name and when it was last opened.
-->
<script lang="ts">
  import type { RecentProject } from '$lib/ipc/bindings';
  import { describeDate } from '$lib/i18n/format';
  import { i18n } from '$lib/i18n/index.svelte';
  import { formatDuration } from '$lib/time/time';

  import EdgePrint from './EdgePrint.svelte';
  import { aspectLabel } from './ratio';

  interface Props {
    project: RecentProject;
    frame: number;
    now: number;
    onopen: (project: RecentProject) => void;
  }

  let { project, frame, now, onopen }: Props = $props();

  const ratio = $derived(project.format ? aspectLabel(project.format) : '16:9');
  /** Width over height; the print is sized from it within its cell. */
  const aspect = $derived(project.format ? project.format.width / project.format.height : 16 / 9);
  const duration = $derived(
    project.duration ? formatDuration(project.duration) : i18n.t('start.unknownDuration'),
  );
  const opened = $derived.by(() => {
    const date = describeDate(project.openedAt, now, i18n.locale);
    return typeof date === 'string' ? date : i18n.message(date);
  });
</script>

<button
  type="button"
  class="print"
  aria-disabled={!project.available}
  title={project.path}
  onclick={() => project.available && onopen(project)}
>
  <span class="stage" aria-hidden="true">
    <span class="paper" style:--aspect={aspect}><span class="image"></span></span>
  </span>
  <EdgePrint
    {frame}
    {ratio}
    {duration}
    status={project.available ? undefined : i18n.t('start.missingProject')}
  />
  <span class="name">{project.name}</span>
  <span class="date">{opened}</span>
</button>

<style>
  .print {
    display: grid;
    gap: var(--space-1);
    width: 100%;
    min-width: 0;
    padding: var(--space-2);
    border: var(--hairline) solid transparent;
    border-radius: var(--radius-control);
    background: transparent;
    text-align: left;
    transition:
      border-color var(--duration-fast) var(--ease-out),
      background-color var(--duration-fast) var(--ease-out);
  }

  .print:hover:not([aria-disabled='true']) {
    border-color: var(--border-subtle);
    background: var(--surface-raised);
  }

  /* One scale, one baseline: every print is the same height, standing on
     the same line, whatever its ratio. */
  .stage {
    container-type: size;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    height: 7.5rem;
    margin-bottom: var(--space-2);
  }

  /* As tall as the stage, unless the cell is too narrow for the ratio. */
  .paper {
    display: block;
    width: min(100cqw, 100cqh * var(--aspect));
    aspect-ratio: var(--aspect);
    padding: var(--print-border);
    background: var(--surface-print);
    border-radius: var(--radius-print);
    box-shadow: var(--shadow-lift);
  }

  .image {
    display: block;
    width: 100%;
    height: 100%;
    background: var(--surface-unexposed);
  }

  [aria-disabled='true'] .paper {
    opacity: 0.45;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  [aria-disabled='true'] .name {
    color: var(--text-secondary);
  }

  .date {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }
</style>
