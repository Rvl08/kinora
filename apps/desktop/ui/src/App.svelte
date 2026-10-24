<!--
  Application frame. With no project open it shows the start screen; an
  open project shows the workspace (an honest placeholder until the Phase 1
  editor). Routes drops and the new-project draft, and moves focus when the
  view changes.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';

  import StartScreen from '$lib/features/start/StartScreen.svelte';
  import { type Draft, newDraft } from '$lib/features/start/draft';
  import { type RatioPreset, settingsFor } from '$lib/features/start/ratio';
  import Workspace from '$lib/features/workspace/Workspace.svelte';
  import { IpcClient } from '$lib/ipc/client';
  import { createRuntime } from '$lib/ipc/runtime';
  import type { Unlisten } from '$lib/ipc/transport';
  import { i18n } from '$lib/i18n/index.svelte';
  import { classifyDrop, droppedPath, type DroppedItem } from '$lib/platform/drop';
  import { type CreateOutcome, Session } from '$lib/state/session.svelte';

  let session = $state.raw<Session | null>(null);
  let draft = $state.raw<Draft | null>(null);
  let nativeDrag = $state(false);
  const now = Date.now();

  onMount(() => {
    let disposed = false;
    const stops: Unlisten[] = [];
    const keep = (stop: Unlisten) => (disposed ? stop() : stops.push(stop));

    void (async () => {
      const runtime = await createRuntime();
      if (disposed) return;
      const current = new Session(new IpcClient(runtime.transport), runtime.platform);
      session = current;
      keep(await current.start());
      const watching = runtime.platform.watchDrops({
        hover: (active) => (nativeDrag = active && current.view === 'start'),
        drop: (paths) => drop(paths.map(droppedPath)),
      });
      if (watching) {
        try {
          keep(await watching);
        } catch (reason) {
          console.error('Kinora: drag and drop is unavailable', reason);
        }
      }
    })();

    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  });

  // Focus follows the view: the project's name once it opens, the start
  // heading once it closes. The first render keeps the browser's default.
  let shown: 'start' | 'project' | null = null;
  $effect(() => {
    const view = session?.view ?? null;
    if (view === null || view === shown) return;
    const first = shown === null;
    shown = view;
    if (view === 'project') draft = null;
    if (first) return;
    void tick().then(() =>
      document.getElementById(view === 'project' ? 'project-title' : 'new-title')?.focus(),
    );
  });

  function begin(preset: RatioPreset, video: string | null = null) {
    draft = newDraft(
      preset,
      { prefix: i18n.t('start.form.defaultPrefix'), now: Date.now(), locale: i18n.locale },
      video,
    );
  }

  async function create(name: string): Promise<CreateOutcome> {
    if (!session || !draft) return { status: 'busy' };
    return session.createProject(name, settingsFor(draft.preset), draft.skippedVideo);
  }

  async function cancelDraft() {
    const preset = draft?.preset;
    draft = null;
    await tick();
    document.querySelector<HTMLElement>(`.ratios [data-preset="${preset}"]`)?.focus();
  }

  /** A drop on the start screen: open a project folder, or start a project for a video. */
  function drop(items: readonly DroppedItem[]) {
    if (!session || session.view !== 'start' || session.activity !== null) return;
    const intent = classifyDrop(items);
    if (intent?.kind === 'project') void session.openDropped(intent.path);
    else if (intent?.kind === 'video') begin('vertical', intent.name);
    else if (intent) {
      session.showNotice({ key: 'start.notice.unsupportedDrop', params: { file: intent.name } });
    }
  }

  async function chooseVideo() {
    const path = await session?.chooseVideo();
    if (path) drop([droppedPath(path)]);
  }
</script>

{#if session}
  {#if session.view === 'project'}
    <Workspace {session} />
  {:else}
    <StartScreen
      {session}
      {now}
      {draft}
      {nativeDrag}
      onbegin={(preset) => begin(preset)}
      oncreate={create}
      oncanceldraft={cancelDraft}
      ondrop={drop}
      onchoosevideo={chooseVideo}
    />
  {/if}
{/if}
