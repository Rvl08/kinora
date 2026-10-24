// The app session: which project is open, and the intents that open,
// create, save and close projects. Each intent is one short sequence of
// native dialogs and shell commands; only one runs at a time, so a double
// click or a held shortcut cannot start two.

import type { IpcError, ProjectOpened, ProjectSettings, RecentProject } from '$lib/ipc/bindings';
import type { IpcClient } from '$lib/ipc/client';
import { toFailure } from '$lib/ipc/errors';
import type { Unlisten } from '$lib/ipc/transport';
import { i18n, type MessageRef } from '$lib/i18n/index.svelte';
import type { FolderDialogOptions, Platform } from '$lib/platform/platform';

import { ProjectStore } from './project.svelte';
import { RecentProjectsStore } from './recent.svelte';

/** What the session is busy with; `null` when idle. */
export type Activity = 'choosing' | 'opening' | 'creating' | 'forgetting' | 'saving' | 'closing';

/** Something that failed, to show until dismissed or until the next intent. */
export interface Notice {
  /** What failed, e.g. "The project could not be opened." */
  readonly message: MessageRef;
  /** Why, as the shell reported it; described with `describeError`. */
  readonly error: IpcError | null;
}

/** What the person should know about the project that just opened. */
export interface OpenedNotes {
  /** The format version it was migrated from, if it was. */
  readonly migratedFrom: number | null;
  /** Unsaved edits replayed from the journal of an unclean shutdown. */
  readonly recoveredEdits: number;
  /** A dropped video that could not be added (no media import yet). */
  readonly skippedVideo: string | null;
}

export type CreateOutcome =
  | { readonly status: 'created' }
  | { readonly status: 'cancelled' }
  | { readonly status: 'busy' }
  | { readonly status: 'failed'; readonly error: IpcError };

export class Session {
  readonly project: ProjectStore;
  readonly recent: RecentProjectsStore;
  readonly #client: IpcClient;
  readonly #platform: Platform;
  #activity = $state<Activity | null>(null);
  #notice = $state.raw<Notice | null>(null);
  #notes = $state.raw<OpenedNotes | null>(null);
  #confirmingClose = $state(false);
  /** The folder the last new project went into: where dialogs start. A dialog hint only. */
  #lastFolder: string | undefined;

  constructor(client: IpcClient, platform: Platform) {
    this.#client = client;
    this.#platform = platform;
    this.project = new ProjectStore(client);
    this.recent = new RecentProjectsStore(client);
  }

  /** `project` while a project is open, else `start`. */
  get view(): 'start' | 'project' {
    return this.project.state ? 'project' : 'start';
  }

  get activity(): Activity | null {
    return this.#activity;
  }

  get notice(): Notice | null {
    return this.#notice;
  }

  get notes(): OpenedNotes | null {
    return this.#notes;
  }

  /** The project has unsaved changes and the person asked to close it. */
  get confirmingClose(): boolean {
    return this.#confirmingClose;
  }

  /** Follows the shell and loads the recent projects; resolves to a function that stops following. */
  async start(): Promise<Unlisten> {
    const stops: Unlisten[] = [];
    try {
      stops.push(await this.project.connect());
      // Closed elsewhere (e.g. the local API): back to a fresh list.
      stops.push(
        await this.#client.on('project:changed', (event) => {
          // A close from this window is finished by `#close` itself.
          if (event.reason === 'closed' && this.#activity !== 'closing') this.#afterClose();
        }),
      );
    } catch (reason) {
      console.error('Kinora: could not follow project changes', reason);
    }
    void this.recent.load();
    return () => stops.forEach((stop) => stop());
  }

  dismissNotice(): void {
    this.#notice = null;
  }

  /** Shows a problem the UI found itself (e.g. a dropped photo). */
  showNotice(message: MessageRef): void {
    this.#notice = { message, error: null };
  }

  /** "Choose a video…": the native file dialog. Resolves to the path, or `null`. */
  async chooseVideo(): Promise<string | null> {
    let path: string | null = null;
    await this.#exclusive('choosing', async () => {
      try {
        path = await this.#platform.pickVideo({
          title: i18n.t('dialogs.chooseVideo'),
          filterName: i18n.t('dialogs.videoFilter'),
        });
      } catch (reason) {
        this.#notice = {
          message: { key: 'start.notice.dialogFailed' },
          error: toFailure(reason).error,
        };
      }
    });
    return path;
  }

  /** Ctrl+O / "Open project…": a folder dialog, then `open_project`. */
  openProject(): Promise<void> {
    return this.#exclusive('choosing', async () => {
      const path = await this.#pick({
        purpose: 'openProject',
        title: i18n.t('dialogs.openProject'),
      });
      if (path !== null) await this.#open(path, { key: 'start.notice.openFailed' });
    });
  }

  /** A project on the contact sheet. */
  openRecent(project: RecentProject): Promise<void> {
    return this.#exclusive('opening', async () => {
      await this.#open(project.path, { key: 'start.notice.openFailed' });
    });
  }

  /** A `.kinora` folder dropped on the window. */
  openDropped(path: string): Promise<void> {
    return this.#exclusive('opening', async () => {
      await this.#open(path, { key: 'start.notice.openFailed' });
    });
  }

  /**
   * "Find folder…" for a project whose folder moved: the person picks the
   * folder, it opens, and only then is the old entry removed from the list.
   */
  locateRecent(project: RecentProject): Promise<void> {
    return this.#exclusive('choosing', async () => {
      const path = await this.#pick({
        purpose: 'locateProject',
        title: i18n.t('dialogs.locateProject', { name: project.name }),
      });
      if (path === null) return;
      if (!(await this.#open(path, { key: 'start.notice.locateFailed' }))) return;
      // The same entry is now valid again; forgetting it would drop the open project.
      if (path === project.path) return;
      try {
        await this.recent.forget(project);
      } catch (reason) {
        // The project is open; a stale entry left in the list is harmless and
        // still offers "Remove from list" later.
        console.warn('Kinora: could not remove the old recent entry', reason);
      }
    });
  }

  /** "Remove from list": the folder itself is not touched. */
  forgetRecent(project: RecentProject): Promise<void> {
    return this.#exclusive('forgetting', async () => {
      try {
        await this.recent.forget(project);
      } catch (reason) {
        this.#notice = {
          message: { key: 'start.notice.forgetFailed' },
          error: toFailure(reason).error,
        };
      }
    });
  }

  /**
   * Creates a project named `name`: asks where, then `create_project`.
   * Failures are returned rather than shown, so the form can place them
   * on the name field.
   */
  async createProject(
    name: string,
    settings: ProjectSettings,
    skippedVideo: string | null = null,
  ): Promise<CreateOutcome> {
    let outcome: CreateOutcome = { status: 'busy' };
    await this.#exclusive('choosing', async () => {
      let parentDir: string | null;
      try {
        parentDir = await this.#platform.pickFolder({
          purpose: 'projectLocation',
          title: i18n.t('dialogs.projectLocation'),
          defaultPath: this.#lastFolder,
        });
      } catch (reason) {
        outcome = { status: 'failed', error: toFailure(reason).error };
        return;
      }
      if (parentDir === null) {
        outcome = { status: 'cancelled' };
        return;
      }
      this.#lastFolder = parentDir;
      this.#activity = 'creating';
      try {
        const result = await this.#client.call('create_project', { parentDir, name, settings });
        this.#show(result, skippedVideo);
        outcome = { status: 'created' };
      } catch (reason) {
        outcome = { status: 'failed', error: toFailure(reason).error };
      }
    });
    return outcome;
  }

  /** Ctrl+S / "Save". Resolves to whether the project was saved. */
  async save(): Promise<boolean> {
    let saved = false;
    await this.#exclusive('saving', async () => {
      saved = await this.#save();
    });
    return saved;
  }

  /** "Close project": asks first when there are unsaved changes. */
  requestClose(): Promise<void> {
    if (this.project.state?.dirty) {
      this.#confirmingClose = true;
      return Promise.resolve();
    }
    return this.#exclusive('closing', () => this.#close(false));
  }

  /** From the confirmation: save, then close if saving worked. */
  saveAndClose(): Promise<void> {
    return this.#exclusive('saving', async () => {
      if (await this.#save()) await this.#close(false);
    });
  }

  /** From the confirmation: close and lose the unsaved changes. */
  discardAndClose(): Promise<void> {
    return this.#exclusive('closing', () => this.#close(true));
  }

  cancelClose(): void {
    this.#confirmingClose = false;
  }

  /** Runs one intent at a time; the notice of an earlier intent is cleared. */
  async #exclusive(first: Activity, work: () => Promise<void>): Promise<void> {
    if (this.#activity !== null) return;
    this.#activity = first;
    this.#notice = null;
    try {
      await work();
    } finally {
      this.#activity = null;
    }
  }

  /** A folder from the native dialog; `null` when cancelled or when the dialog failed (noticed). */
  async #pick(options: FolderDialogOptions): Promise<string | null> {
    this.#activity = 'choosing';
    try {
      return await this.#platform.pickFolder({ defaultPath: this.#lastFolder, ...options });
    } catch (reason) {
      this.#notice = {
        message: { key: 'start.notice.dialogFailed' },
        error: toFailure(reason).error,
      };
      return null;
    }
  }

  /** `open_project`; on failure shows `failed` and resolves to `false`. */
  async #open(path: string, failed: MessageRef): Promise<boolean> {
    this.#activity = 'opening';
    try {
      this.#show(await this.#client.call('open_project', { path }), null);
      return true;
    } catch (reason) {
      const error = toFailure(reason).error;
      this.#notice = { message: failed, error };
      // The folder may have gone since the list was loaded.
      if (error.code === 'io' && error.kind === 'notFound') void this.recent.load();
      return false;
    }
  }

  async #save(): Promise<boolean> {
    this.#activity = 'saving';
    try {
      await this.project.save();
      return true;
    } catch (reason) {
      this.#notice = { message: { key: 'workspace.saveFailed' }, error: toFailure(reason).error };
      return false;
    }
  }

  async #close(discardChanges: boolean): Promise<void> {
    this.#activity = 'closing';
    try {
      await this.project.close(discardChanges);
      this.#afterClose();
    } catch (reason) {
      const error = toFailure(reason).error;
      if (error.code === 'unsavedChanges') {
        this.#confirmingClose = true;
      } else {
        this.#confirmingClose = false;
        this.#notice = { message: { key: 'workspace.closeFailed' }, error };
      }
    }
  }

  #show(result: ProjectOpened, skippedVideo: string | null): void {
    this.project.opened(result);
    this.#notes = {
      migratedFrom: result.migratedFrom,
      recoveredEdits: result.recoveredEdits,
      skippedVideo,
    };
  }

  #afterClose(): void {
    this.#confirmingClose = false;
    this.#notes = null;
    void this.recent.load();
  }
}
