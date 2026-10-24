<!--
  The one step between choosing a ratio and having a project: its name. The
  chosen shape stays in view; Enter asks where to put it (native dialog) and
  creates it. The suggested name is selected, so typing replaces it.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import type { IpcError } from '$lib/ipc/bindings';
  import { describeError } from '$lib/ipc/errors';
  import { i18n } from '$lib/i18n/index.svelte';
  import type { Activity, CreateOutcome } from '$lib/state/session.svelte';

  import {
    checkProjectName,
    createErrorTarget,
    describeNameError,
    normaliseName,
  } from './project-name';
  import { presetLabel, type RatioPreset } from './ratio';

  interface Props {
    preset: RatioPreset;
    /** The suggested name; edited freely afterwards. */
    initialName: string;
    /** A dropped video that will not be added (no media import yet). */
    skippedVideo: string | null;
    activity: Activity | null;
    oncreate: (name: string) => Promise<CreateOutcome>;
    oncancel: () => void;
  }

  let { preset, initialName, skippedVideo, activity, oncreate, oncancel }: Props = $props();

  let input: HTMLInputElement | undefined = $state();
  let name = $state(untrack(() => initialName));
  /** Checks run once the person tried to submit, not while they first type. */
  let checked = $state(false);
  let refused = $state.raw<IpcError | null>(null);

  const busy = $derived(activity !== null);
  const localError = $derived(checked ? checkProjectName(name) : null);
  const nameError = $derived(localError ?? (refused ? describeNameError(refused) : null));
  const formError = $derived(refused && createErrorTarget(refused) === 'form' ? refused : null);
  const describedBy = $derived(
    ['project-name-hint', nameError ? 'project-name-error' : ''].filter(Boolean).join(' '),
  );

  onMount(() => {
    input?.focus();
    input?.select();
  });

  function edited() {
    refused = null;
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    checked = true;
    refused = null;
    if (checkProjectName(name) !== null) {
      input?.focus();
      return;
    }
    const outcome = await oncreate(normaliseName(name));
    if (outcome.status !== 'failed') return;
    refused = outcome.error;
    if (createErrorTarget(outcome.error) === 'name') {
      input?.focus();
      input?.select();
    }
  }

  // Escape in the name field steps back to the ratio choice.
  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && !busy) {
      event.preventDefault();
      oncancel();
    }
  }
</script>

<form class="draft" aria-labelledby="draft-title" aria-busy={busy} novalidate onsubmit={submit}>
  <div class="shape">
    <span class="frame" data-preset={preset} aria-hidden="true">
      <span class="value">{presetLabel(preset)}</span>
    </span>
    <div class="named">
      <h2 id="draft-title">{i18n.t('start.form.heading')}</h2>
      <p class="caption">
        {presetLabel(preset)} · {i18n.t(`start.ratio.${preset}`)} · {i18n.t(
          `start.ratio.${preset}Use`,
        )}
      </p>
    </div>
  </div>

  <div class="field">
    <label for="project-name">{i18n.t('start.form.nameLabel')}</label>
    <input
      id="project-name"
      bind:this={input}
      bind:value={name}
      oninput={edited}
      onkeydown={keydown}
      readonly={busy}
      autocomplete="off"
      spellcheck="false"
      aria-invalid={nameError !== null}
      aria-describedby={describedBy}
    />
    {#if nameError}
      <p id="project-name-error" class="error">{i18n.message(nameError)}</p>
    {/if}
    <p id="project-name-hint" class="hint">{i18n.t('start.form.nameHint')}</p>
  </div>

  {#if skippedVideo}
    <p class="note">{i18n.t('start.form.videoSkipped', { file: skippedVideo })}</p>
  {/if}

  <div class="status" aria-live="polite">
    {#if formError}
      <p class="error">{i18n.t('start.form.createFailed')}</p>
      <p class="hint">{i18n.message(describeError(formError))}</p>
    {:else if activity === 'choosing'}
      <p class="hint">{i18n.t('start.notice.choosing')}</p>
    {:else if activity === 'creating'}
      <p class="hint">{i18n.t('start.form.creating')}</p>
    {/if}
  </div>

  <div class="actions">
    <Button variant="primary" type="submit" disabled={busy}>{i18n.t('start.form.create')}</Button>
    <Button variant="quiet" disabled={busy} onclick={oncancel}>{i18n.t('start.form.cancel')}</Button
    >
  </div>
</form>

<style>
  .draft {
    display: grid;
    gap: var(--space-5);
  }

  /* The chosen shape, drawn as on the ratio row but selected, not filled. */
  .shape {
    display: flex;
    align-items: flex-end;
    gap: var(--space-4);
  }

  .frame {
    display: flex;
    flex: none;
    align-items: flex-end;
    height: 6rem;
    padding: var(--space-2);
    border: 2px solid var(--accent);
    border-radius: var(--radius-print);
  }

  [data-preset='vertical'] {
    aspect-ratio: 9 / 16;
  }

  [data-preset='landscape'] {
    aspect-ratio: 16 / 9;
  }

  [data-preset='square'] {
    aspect-ratio: 1;
  }

  .value {
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 700;
    font-size: var(--text-md);
    line-height: 1;
    font-variant-numeric: tabular-nums;
    color: var(--text-accent);
  }

  .named {
    display: grid;
    gap: var(--space-1);
    min-width: 0;
  }

  h2 {
    font-family: var(--font-display);
    font-stretch: 87.5%;
    font-weight: 650;
    font-size: var(--text-lg);
  }

  .caption {
    font-family: var(--font-display);
    font-stretch: var(--edge-stretch);
    font-weight: 600;
    font-size: var(--text-xs);
    letter-spacing: var(--edge-tracking);
    text-transform: uppercase;
    font-variant-numeric: tabular-nums;
    color: var(--text-tertiary);
  }

  .field {
    display: grid;
    gap: var(--space-2);
  }

  label {
    font-weight: 600;
  }

  input {
    min-height: 2.25rem;
    padding: 0 var(--space-3);
    border: var(--hairline) solid var(--border-control);
    border-radius: var(--radius-control);
    background: var(--surface-base);
    color: var(--text-primary);
    font-size: var(--text-md);
    transition: border-color var(--duration-fast) var(--ease-out);
  }

  input:hover:not(:focus-visible) {
    border-color: var(--text-tertiary);
  }

  input[aria-invalid='true'] {
    border-color: var(--status-danger);
  }

  input[readonly] {
    color: var(--text-secondary);
  }

  .hint {
    color: var(--text-secondary);
  }

  .note {
    padding: var(--space-3) 0;
    border-block: var(--hairline) solid var(--border-subtle);
    color: var(--text-primary);
  }

  .error {
    font-weight: 600;
    color: var(--status-danger);
  }

  .status:empty {
    display: none;
  }

  .status {
    display: grid;
    gap: var(--space-1);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
</style>
