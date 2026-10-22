<!--
  Application frame. With no project open it shows the start screen; the
  editor workspace replaces it once the shell opens a project (Phase 1).
-->
<script lang="ts">
  import { onMount } from 'svelte';

  import StartScreen from '$lib/features/start/StartScreen.svelte';
  import type { RatioPreset } from '$lib/features/start/ratio';
  import type { RecentProject } from '$lib/ipc/bindings';
  import { IpcClient } from '$lib/ipc/client';
  import { createTransport } from '$lib/ipc/runtime';
  import { RecentProjectsStore } from '$lib/state/recent.svelte';

  let recent = $state.raw<RecentProjectsStore | null>(null);
  const now = Date.now();

  onMount(() => {
    void createTransport().then((transport) => {
      const store = new RecentProjectsStore(new IpcClient(transport));
      recent = store;
      void store.load();
    });
  });

  // The shell commands behind these intents (create, open, locate, forget,
  // file dialogs) arrive with the Tauri shell; the start screen already
  // emits them.
  function create(preset: RatioPreset) {
    void preset;
  }
  function open() {}
  function openRecent(project: RecentProject) {
    void project;
  }
  function locateRecent(project: RecentProject) {
    void project;
  }
  function forgetRecent(project: RecentProject) {
    void project;
  }
  function dropMedia(files: readonly File[]) {
    void files;
  }
  function chooseVideo() {}
</script>

{#if recent}
  <StartScreen
    {recent}
    {now}
    oncreate={create}
    onopen={open}
    onopenrecent={openRecent}
    onlocaterecent={locateRecent}
    onforgetrecent={forgetRecent}
    ondropmedia={dropMedia}
    onchoosevideo={chooseVideo}
  />
{/if}
