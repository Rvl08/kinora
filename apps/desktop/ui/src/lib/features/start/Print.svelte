<!--
  One recent project on the contact sheet: the print in its own ratio over
  the edge print, then the name. A project whose folder is missing offers
  its recovery instead of opening.
-->
<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import type { RecentProject } from '$lib/ipc/bindings';
  import { describeDate } from '$lib/i18n/format';
  import { i18n } from '$lib/i18n/index.svelte';
  import { formatDuration } from '$lib/time/time';

  import EdgePrint from '$lib/components/EdgePrint.svelte';
  import { aspectLabel } from './ratio';

  interface Props {
    project: RecentProject;
    frame: number;
    now: number;
    onopen: (project: RecentProject) => void;
    onlocate: (project: RecentProject) => void;
    onforget: (project: RecentProject) => void;
  }

  let { project, frame, now, onopen, onlocate, onforget }: Props = $props();

  const ratio = $derived(project.format ? aspectLabel(project.format) : '');
  /** Width over height; the print is sized from it within its cell. */
  const aspect = $derived(project.format ? project.format.width / project.format.height : 16 / 9);
  const duration = $derived(
    project.duration ? formatDuration(project.duration) : i18n.t('start.unknownDuration'),
  );
  const opened = $derived.by(() => {
    const date = describeDate(project.openedAt, now, i18n.locale);
    return typeof date === 'string' ? date : i18n.message(date);
  });
  /** The folder that held the project, for a missing one. */
  const parent = $derived(project.path.replace(/[\\/][^\\/]*$/, ''));
</script>

{#snippet print()}
  <span class="stage" aria-hidden="true">
    <span class="paper" style:--aspect={aspect}><span class="image"></span></span>
  </span>
{/snippet}

{#if project.available}
  <button type="button" class="print" onclick={() => onopen(project)}>
    {@render print()}
    <EdgePrint {frame} {ratio} date={opened} {duration} />
    <span class="name">{project.name}</span>
  </button>
{:else}
  <div class="print missing">
    {@render print()}
    <EdgePrint {frame} status={i18n.t('start.missingProject')} />
    <span class="name">{project.name}</span>
    <span class="where">{i18n.t('start.lastLocation', { path: parent })}</span>
    <span class="actions">
      <Button variant="secondary" onclick={() => onlocate(project)}>
        {i18n.t('start.locateProject')}
      </Button>
      <Button variant="secondary" onclick={() => onforget(project)}>
        {i18n.t('start.forgetProject')}
      </Button>
    </span>
  </div>
{/if}

<style>
  .print {
    display: grid;
    align-content: start;
    width: 100%;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    text-align: left;
  }

  /* One scale, one baseline: every print is the same height, standing on
     the same line, whatever its ratio. */
  .stage {
    container-type: size;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    height: 7.5rem;
    margin: 0 var(--space-3) var(--space-3);
  }

  .paper {
    display: block;
    width: min(100cqw, 100cqh * var(--aspect));
    aspect-ratio: var(--aspect);
    padding: var(--print-border);
    background: var(--surface-print);
    border-radius: var(--radius-print);
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .image {
    display: block;
    width: 100%;
    height: 100%;
    background: var(--surface-unexposed);
  }

  button.print:hover .paper,
  button.print:focus-visible .paper {
    background: var(--text-primary);
  }

  .name {
    padding: var(--space-2) var(--space-3) 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  button.print:hover .name {
    color: var(--text-accent);
  }

  .missing .paper {
    opacity: 0.4;
  }

  .missing .name {
    color: var(--text-secondary);
  }

  .where {
    padding: 0 var(--space-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3) 0;
  }
</style>
