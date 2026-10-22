<!--
  "Terakhir dibuka": recent projects laid out as a contact sheet, with
  loading, error and first-run states.
-->
<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import type { RecentProject } from '$lib/ipc/bindings';
  import { describeError } from '$lib/ipc/errors';
  import { i18n } from '$lib/i18n/index.svelte';
  import type { RecentProjectsStore } from '$lib/state/recent.svelte';

  import EdgePrint from './EdgePrint.svelte';
  import Print from './Print.svelte';

  interface Props {
    recent: RecentProjectsStore;
    now: number;
    onopen: (project: RecentProject) => void;
  }

  let { recent, now, onopen }: Props = $props();

  /** Frames of the unexposed strip shown before the first project. */
  const BLANK_FRAMES = [1, 2, 3, 4];
</script>

<section class="sheet" aria-labelledby="recent-title" aria-busy={recent.status === 'loading'}>
  <header>
    <h2 id="recent-title">{i18n.t('start.recentProjects')}</h2>
    {#if recent.status === 'ready' && recent.items.length > 0}
      <span class="count">{i18n.plural('units.projects', recent.items.length)}</span>
    {/if}
  </header>

  <div class="body" aria-live="polite">
    {#if recent.status === 'loading'}
      <p class="note">{i18n.t('start.loadingRecent')}</p>
    {:else if recent.status === 'error'}
      <div class="problem">
        <p>{i18n.t('start.recentError')}</p>
        {#if recent.error}<p class="detail">{i18n.message(describeError(recent.error))}</p>{/if}
        <Button variant="secondary" onclick={() => recent.load()}>{i18n.t('start.retry')}</Button>
      </div>
    {:else if recent.items.length === 0}
      <div class="unexposed">
        <ol class="strip" aria-hidden="true">
          {#each BLANK_FRAMES as frame (frame)}
            <li>
              <span class="blank"></span>
              <EdgePrint {frame} ratio="" duration="" />
            </li>
          {/each}
        </ol>
        <p class="note">{i18n.t('start.noRecentProjects')}</p>
      </div>
    {:else}
      <ul class="grid">
        {#each recent.items as project, index (project.path)}
          <li><Print {project} frame={index + 1} {now} {onopen} /></li>
        {/each}
      </ul>
    {/if}
  </div>
</section>

<style>
  .sheet {
    display: grid;
    grid-template-rows: auto 1fr;
    gap: var(--space-4);
    min-height: 0;
    padding: var(--space-6);
    background: var(--surface-sunken);
  }

  header {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }

  h2 {
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .count {
    color: var(--text-tertiary);
  }

  .body {
    min-height: 0;
    overflow-y: auto;
  }

  .grid {
    display: grid;
    /* Wide enough for a 16:9 print at the shared print height. */
    grid-template-columns: repeat(auto-fill, minmax(14.5rem, 1fr));
    gap: var(--space-4) var(--space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .note {
    color: var(--text-secondary);
  }

  .problem {
    display: grid;
    justify-items: start;
    gap: var(--space-2);
  }

  .detail {
    color: var(--text-tertiary);
  }

  .unexposed {
    display: grid;
    gap: var(--space-5);
    max-width: 44rem;
  }

  .strip {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .blank {
    display: block;
    aspect-ratio: 9 / 16;
    max-height: 7.5rem;
    margin: 0 auto var(--space-3);
    border: var(--hairline) solid var(--border-subtle);
    border-radius: var(--radius-print);
  }
</style>
