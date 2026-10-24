<!--
  The start screen: start a project (a ratio, or a video dropped anywhere on
  the window), open one, or pick up a recent one from the contact sheet.
  Choosing a ratio swaps the start column for the naming step.
-->
<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';

  import Button from '$lib/components/Button.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import QuickSettings from '$lib/features/settings/QuickSettings.svelte';
  import type { RecentProject } from '$lib/ipc/bindings';
  import { describeError } from '$lib/ipc/errors';
  import { i18n } from '$lib/i18n/index.svelte';
  import { ariaShortcut, dispatch, shortcutLabel } from '$lib/keyboard/shortcuts';
  import type { DroppedItem } from '$lib/platform/drop';
  import type { CreateOutcome, Session } from '$lib/state/session.svelte';

  import ContactSheet from './ContactSheet.svelte';
  import type { Draft } from './draft';
  import DropBand from './DropBand.svelte';
  import NewProjectForm from './NewProjectForm.svelte';
  import RatioButton from './RatioButton.svelte';
  import { RATIO_PRESETS, type RatioPreset } from './ratio';

  interface Props {
    session: Session;
    now: number;
    draft: Draft | null;
    /** Files are dragged over the window (native drag and drop, inside the shell). */
    nativeDrag: boolean;
    onbegin: (preset: RatioPreset) => void;
    oncreate: (name: string) => Promise<CreateOutcome>;
    oncanceldraft: () => void;
    ondrop: (items: readonly DroppedItem[]) => void;
    onchoosevideo: () => void;
  }

  let {
    session,
    now,
    draft,
    nativeDrag,
    onbegin,
    oncreate,
    oncanceldraft,
    ondrop,
    onchoosevideo,
  }: Props = $props();

  function shortcuts(event: KeyboardEvent) {
    dispatch(event, {
      newProject: () => onbegin('vertical'),
      openProject: () => void session.openProject(),
    });
  }

  // In a browser the page's own drag events carry drops (names only). Inside
  // the shell, native drag and drop takes over and `nativeDrag` reports it.
  // dragenter/dragleave fire for every child element, so count them.
  let htmlDrag = $state(false);
  let depth = 0;
  const dragging = $derived(htmlDrag || nativeDrag);

  const hasFiles = (event: DragEvent) => event.dataTransfer?.types.includes('Files') ?? false;

  function dragenter(event: DragEvent) {
    if (!hasFiles(event)) return;
    event.preventDefault();
    depth += 1;
    htmlDrag = true;
  }

  function dragover(event: DragEvent) {
    if (!hasFiles(event)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
  }

  function dragleave() {
    depth = Math.max(0, depth - 1);
    if (depth === 0) htmlDrag = false;
  }

  function drop(event: DragEvent) {
    event.preventDefault();
    depth = 0;
    htmlDrag = false;
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (files.length > 0) ondrop(files.map((file) => ({ name: file.name, path: null })));
  }

  const notice = $derived(session.notice);

  /** The removed print took focus with it; hand it to the sheet's heading. */
  async function forget(project: RecentProject) {
    await session.forgetRecent(project);
    if (!session.notice) document.getElementById('recent-title')?.focus();
  }
</script>

<svelte:window onkeydown={shortcuts} />

<div
  class="start"
  role="presentation"
  ondragenter={dragenter}
  ondragover={dragover}
  ondragleave={dragleave}
  ondrop={drop}
>
  <header class="bar">
    <p class="mark">Kinora</p>
    <QuickSettings />
  </header>

  <main class="layout">
    <section class="begin" aria-labelledby="new-title" aria-busy={session.activity !== null}>
      <div class="intro">
        <h1 id="new-title" tabindex="-1">{i18n.t('start.newProject')}</h1>
        {#if !draft}<p>{i18n.t('start.newProjectHint')}</p>{/if}
      </div>

      <div class="status" aria-live="polite">
        {#if notice}
          <Notice
            message={i18n.message(notice.message)}
            detail={notice.error ? i18n.message(describeError(notice.error)) : undefined}
            dismissLabel={i18n.t('start.notice.dismiss')}
            ondismiss={() => session.dismissNotice()}
          />
        {:else if !draft && session.activity === 'opening'}
          <p class="working">{i18n.t('start.notice.opening')}</p>
        {:else if !draft && session.activity === 'choosing'}
          <p class="working">{i18n.t('start.notice.choosing')}</p>
        {/if}
      </div>

      {#if draft}
        {#key draft}
          <NewProjectForm
            preset={draft.preset}
            initialName={draft.name}
            skippedVideo={draft.skippedVideo}
            activity={session.activity}
            {oncreate}
            oncancel={oncanceldraft}
          />
        {/key}
      {:else}
        <div class="ratios">
          {#each RATIO_PRESETS as preset, index (preset)}
            <RatioButton
              {preset}
              primary={index === 0}
              shortcut={index === 0 ? 'n' : undefined}
              showUse={index !== 0}
              onselect={onbegin}
            />
          {/each}
        </div>
        <p class="hint">{i18n.t('start.verticalHint')}</p>

        <DropBand active={dragging} onchoose={onchoosevideo} />

        <div class="open">
          <Button
            variant="secondary"
            aria-keyshortcuts={ariaShortcut('openProject')}
            shortcut={shortcutLabel('openProject', i18n.t('keys.ctrl'))}
            onclick={() => session.openProject()}
          >
            {#snippet icon()}<FolderOpen size={16} strokeWidth={1.75} />{/snippet}
            {i18n.t('start.openProject')}
          </Button>
        </div>
      {/if}
    </section>

    <ContactSheet
      recent={session.recent}
      {now}
      onopen={(project) => session.openRecent(project)}
      onlocate={(project) => session.locateRecent(project)}
      onforget={forget}
    />
  </main>
</div>

<style>
  .start {
    display: grid;
    grid-template-rows: auto 1fr;
    height: 100%;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
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

  .layout {
    display: grid;
    grid-template-columns: minmax(22rem, 38%) 1fr;
    min-height: 0;
  }

  .begin {
    container-type: inline-size;
    display: grid;
    align-content: start;
    gap: var(--space-5);
    padding: var(--space-6);
    overflow-y: auto;
  }

  .intro {
    display: grid;
    gap: var(--space-2);
  }

  h1 {
    font-family: var(--font-display);
    font-stretch: 87.5%;
    font-size: var(--text-2xl);
    font-weight: 700;
    line-height: var(--leading-tight);
    letter-spacing: -0.005em;
  }

  .intro p {
    color: var(--text-secondary);
  }

  /* Empty until something is reported; then it sits under the heading. */
  .status:empty {
    display: none;
  }

  .working {
    color: var(--text-secondary);
  }

  /* Frames share one height and stand on one baseline. The height fits
     the column: 9/16 + 16/9 + 1 = 3.34 heights of width, plus two gaps and
     some air on the right. */
  .ratios {
    --ratio-height: min(10rem, calc((100cqi - 3rem) / 3.34));
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }

  .hint {
    margin-top: calc(-1 * var(--space-3));
    color: var(--text-secondary);
  }

  .open {
    display: flex;
  }
</style>
