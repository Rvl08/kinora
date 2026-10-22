// The open project as the UI sees it: a read-model of the shell's Editor
// (docs/07 §4). The UI never changes the document itself; it sends
// commands and pulls the document when the shell reports a newer revision.

import type {
  EditCommand,
  Project,
  ProjectChanged,
  ProjectOpened,
  ProjectState,
} from '$lib/ipc/bindings';
import type { IpcClient } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/errors';
import type { Unlisten } from '$lib/ipc/transport';

/** No document has been loaded. */
const NONE = -1;

export class ProjectStore {
  readonly #client: IpcClient;
  // Raw state: each revision replaces the whole document; no deep proxies.
  #document = $state.raw<Project | null>(null);
  #state = $state.raw<ProjectState | null>(null);
  /** Revision of `#document`. */
  #loaded = NONE;
  /** Newest revision the shell has reported. */
  #wanted = NONE;
  #pulling: Promise<void> | null = null;

  constructor(client: IpcClient) {
    this.#client = client;
  }

  /** The document at `state.revision`, or `null` when no project is open. */
  get document(): Project | null {
    return this.#document;
  }

  /** Revision, unsaved changes and undo/redo labels, matching `document`. */
  get state(): ProjectState | null {
    return this.#state;
  }

  /** Follows `project:changed` events until the returned function is called. */
  connect(): Promise<Unlisten> {
    return this.#client.on('project:changed', (event) => void this.#changed(event));
  }

  /** Shows a project that was just created or opened. */
  opened(result: ProjectOpened): void {
    this.#show(result.snapshot.document, result.snapshot.state);
  }

  /** Applies an edit as one undo step. @throws IpcFailure when refused. */
  apply(command: EditCommand): Promise<void> {
    return this.#edit((baseRevision) =>
      this.#client.call('apply_command', { baseRevision, command }),
    );
  }

  /** Reverts the last step. @throws IpcFailure (e.g. `nothingToUndo`). */
  undo(): Promise<void> {
    return this.#edit((baseRevision) => this.#client.call('undo', { baseRevision }));
  }

  /** Re-applies the last undone step. @throws IpcFailure (e.g. `nothingToRedo`). */
  redo(): Promise<void> {
    return this.#edit((baseRevision) => this.#client.call('redo', { baseRevision }));
  }

  /** Fetches the current document, whatever revision the UI holds. */
  async refresh(): Promise<void> {
    const snapshot = await this.#client.call('get_project', null);
    this.#show(snapshot.document, snapshot.state);
  }

  async #edit(send: (baseRevision: number) => Promise<ProjectState>): Promise<void> {
    const current = this.#state;
    if (!current) throw new IpcFailure({ code: 'noProject' });
    try {
      await this.#advance(await send(current.revision));
    } catch (error) {
      // The UI was behind: catch up so the user sees the latest state, and
      // let the caller report the refusal. Edits are never retried blindly.
      if (error instanceof IpcFailure && error.error.code === 'revisionConflict') {
        await this.refresh();
      }
      throw error;
    }
  }

  async #changed(event: ProjectChanged): Promise<void> {
    if (event.state === null) {
      this.#clear();
      return;
    }
    await this.#advance(event.state);
  }

  /** Takes a state reported by the shell, pulling the document if it is newer. */
  async #advance(next: ProjectState): Promise<void> {
    if (this.#state && next.project !== this.#state.project) this.#loaded = NONE;
    if (next.revision === this.#loaded) {
      // Same document (e.g. it was saved): only the state changed.
      this.#state = next;
      return;
    }
    this.#wanted = Math.max(this.#wanted, next.revision);
    await this.#pull();
  }

  /** Fetches until the newest reported revision is shown; one request at a time. */
  #pull(): Promise<void> {
    this.#pulling ??= (async () => {
      try {
        while (this.#loaded < this.#wanted) {
          const snapshot = await this.#client.call('get_project', null);
          const before = this.#loaded;
          this.#show(snapshot.document, snapshot.state);
          // The shell always answers with its newest state; stop if it is
          // not newer, so a misbehaving shell cannot make this loop forever.
          if (this.#loaded <= before) break;
        }
      } finally {
        this.#pulling = null;
      }
    })();
    return this.#pulling;
  }

  #show(document: Project, state: ProjectState): void {
    this.#document = document;
    this.#state = state;
    this.#loaded = state.revision;
    this.#wanted = Math.max(this.#wanted, state.revision);
  }

  #clear(): void {
    this.#document = null;
    this.#state = null;
    this.#loaded = NONE;
    this.#wanted = NONE;
  }
}
