<!--
  An open project, before the editor exists. It says plainly what is open
  and what is not built yet, and offers the two things that work: save and
  close. The editor workspace (Phase 1) replaces the body; the bar stays.
-->
<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import EdgePrint from '$lib/components/EdgePrint.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import QuickSettings from '$lib/features/settings/QuickSettings.svelte';
  import { describeError } from '$lib/ipc/errors';
  import { i18n } from '$lib/i18n/index.svelte';
  import { ariaShortcut, dispatch, shortcutLabel } from '$lib/keyboard/shortcuts';
  import type { Session } from '$lib/state/session.svelte';
  import { aspectLabel, fpsLabel } from '$lib/video/format';

  import CloseConfirm from './CloseConfirm.svelte';

  interface Props {
    session: Session;
  }

  let { session }: Props = $props();

  const project = $derived(session.project.document);
  const state = $derived(session.project.state);
  const notes = $derived(session.notes);
  const notice = $derived(session.notice);
  const busy = $derived(session.activity !== null);
  // The canvas: the first timeline's, else the project default.
  const format = $derived(project?.timelines[0]?.format ?? project?.settings.defaultFormat);
  const facts = $derived(
    format
      ? i18n.t('workspace.format', {
          width: String(format.width),
          height: String(format.height),
          fps: fpsLabel(format.frameRate, i18n.locale),
        })
      : '',
  );

  function shortcuts(event: KeyboardEvent) {
    if (session.confirmingClose) return;
    dispatch(event, { saveProject: () => void session.save() });
  }
</script>

<svelte:window onkeydown={shortcuts} />

{#if project && state}
  <div class="workspace">
    <header class="bar">
      <p class="mark">Kinora</p>
      <p class="file">
        <span class="name">{project.name}</span>
        <span class="saved" class:dirty={state.dirty}>
          {state.dirty ? i18n.t('workspace.unsaved') : i18n.t('workspace.saved')}
        </span>
      </p>
      <div class="actions">
        <Button
          variant="secondary"
          disabled={busy}
          aria-keyshortcuts={ariaShortcut('saveProject')}
          shortcut={shortcutLabel('saveProject', i18n.t('keys.ctrl'))}
          onclick={() => session.save()}
        >
          {session.activity === 'saving' ? i18n.t('workspace.saving') : i18n.t('workspace.save')}
        </Button>
        <Button variant="quiet" disabled={busy} onclick={() => session.requestClose()}>
          {i18n.t('workspace.close')}
        </Button>
        <QuickSettings />
      </div>
    </header>

    <main class="field">
      <section class="project" aria-labelledby="project-title">
        {#if format}
          <div class="print" aria-hidden="true">
            <span class="stage">
              <span class="paper" style:--aspect={format.width / format.height}>
                <span class="image"></span>
              </span>
            </span>
            <EdgePrint frame={1} ratio={aspectLabel(format)} date={facts} />
          </div>
        {/if}

        <div class="about">
          <h1 id="project-title" tabindex="-1">{project.name}</h1>
          <p class="facts">{i18n.plural('units.media', project.media.length)}</p>
          <p class="placeholder">{i18n.t('workspace.placeholder')}</p>

          {#if notes && (notes.recoveredEdits > 0 || notes.migratedFrom !== null || notes.skippedVideo)}
            <ul class="notes">
              {#if notes.recoveredEdits > 0}
                <li>{i18n.plural('workspace.recovered', notes.recoveredEdits)}</li>
              {/if}
              {#if notes.migratedFrom !== null}
                <li>{i18n.t('workspace.migrated', { from: notes.migratedFrom })}</li>
              {/if}
              {#if notes.skippedVideo}
                <li>{i18n.t('start.form.videoSkipped', { file: notes.skippedVideo })}</li>
              {/if}
            </ul>
          {/if}

          <div class="status" aria-live="polite">
            {#if notice}
              <Notice
                message={i18n.message(notice.message)}
                detail={notice.error ? i18n.message(describeError(notice.error)) : undefined}
                dismissLabel={i18n.t('start.notice.dismiss')}
                ondismiss={() => session.dismissNotice()}
              />
            {:else if session.activity === 'closing'}
              <p class="working">{i18n.t('workspace.closing')}</p>
            {/if}
          </div>
        </div>
      </section>
    </main>

    {#if session.confirmingClose}
      <CloseConfirm
        {busy}
        onsave={() => session.saveAndClose()}
        ondiscard={() => session.discardAndClose()}
        oncancel={() => session.cancelClose()}
      />
    {/if}
  </div>
{/if}

<style>
  .workspace {
    display: grid;
    grid-template-rows: auto 1fr;
    height: 100%;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-3) var(--space-6);
    border-bottom: var(--hairline) solid var(--border-subtle);
  }

  .mark {
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 800;
    font-size: var(--text-lg);
    letter-spacing: var(--edge-tracking);
    text-transform: uppercase;
  }

  .file {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    min-width: 0;
    margin-right: auto;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  /* Edge-print voice: the state of the file, not a sentence. */
  .saved {
    flex: none;
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 600;
    font-size: var(--text-xs);
    letter-spacing: var(--edge-tracking);
    text-transform: uppercase;
    color: var(--text-tertiary);
  }

  .saved.dirty {
    color: var(--text-accent);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .field {
    min-height: 0;
    padding: var(--space-6);
    overflow-y: auto;
    background: var(--surface-sunken);
  }

  .project {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-6);
    max-width: 60rem;
  }

  .print {
    display: grid;
    flex: none;
    width: 16rem;
  }

  .stage {
    container-type: size;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    height: 15rem;
    margin: 0 var(--space-3) var(--space-3);
  }

  .paper {
    display: block;
    width: min(100cqw, 100cqh * var(--aspect));
    aspect-ratio: var(--aspect);
    padding: var(--print-border);
    background: var(--surface-print);
    border-radius: var(--radius-print);
  }

  .image {
    display: block;
    width: 100%;
    height: 100%;
    background: var(--surface-unexposed);
  }

  .about {
    display: grid;
    flex: 1 1 20rem;
    align-content: start;
    gap: var(--space-3);
    max-width: 38rem;
  }

  h1 {
    font-family: var(--font-display);
    font-stretch: 87.5%;
    font-size: var(--text-2xl);
    font-weight: 700;
    line-height: var(--leading-tight);
    letter-spacing: -0.005em;
    overflow-wrap: anywhere;
  }

  .facts {
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 600;
    font-size: var(--text-xs);
    letter-spacing: var(--edge-tracking);
    text-transform: uppercase;
    font-variant-numeric: tabular-nums;
    color: var(--text-tertiary);
  }

  .placeholder {
    color: var(--text-secondary);
    max-width: 65ch;
  }

  .notes {
    display: grid;
    gap: var(--space-2);
    margin: 0;
    padding: var(--space-3) 0;
    border-block: var(--hairline) solid var(--border-subtle);
    list-style: none;
  }

  .status:empty {
    display: none;
  }

  .working {
    color: var(--text-secondary);
  }
</style>
