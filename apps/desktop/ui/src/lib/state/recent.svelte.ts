// The recent-projects list shown on the start screen.

import type { IpcError, RecentProject } from '$lib/ipc/bindings';
import type { IpcClient } from '$lib/ipc/client';
import { toFailure } from '$lib/ipc/errors';

export type RecentStatus = 'loading' | 'ready' | 'error';

export class RecentProjectsStore {
  readonly #client: IpcClient;
  #items = $state.raw<readonly RecentProject[]>([]);
  #status = $state<RecentStatus>('loading');
  #error = $state.raw<IpcError | null>(null);

  constructor(client: IpcClient) {
    this.#client = client;
  }

  /** Most recent first. */
  get items(): readonly RecentProject[] {
    return this.#items;
  }

  get status(): RecentStatus {
    return this.#status;
  }

  /** Why loading failed, when `status` is `error`. */
  get error(): IpcError | null {
    return this.#error;
  }

  /** Fetches the list; failures are kept in `error`, never thrown. */
  async load(): Promise<void> {
    this.#status = 'loading';
    this.#error = null;
    try {
      this.#items = await this.#client.call('list_recent_projects', null);
      this.#status = 'ready';
    } catch (reason) {
      this.#error = toFailure(reason).error;
      this.#status = 'error';
    }
  }
}
