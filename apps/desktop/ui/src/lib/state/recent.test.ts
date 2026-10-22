import { describe, expect, it } from 'vitest';

import { IpcClient } from '$lib/ipc/client';
import { MemoryTransport } from '$lib/ipc/transport';

import { RecentProjectsStore } from './recent.svelte';

describe('RecentProjectsStore', () => {
  it('loads the list', async () => {
    const transport = new MemoryTransport().handle('list_recent_projects', () => [
      {
        path: 'D:/Proyek/A.kinora',
        name: 'A',
        openedAt: 1,
        format: null,
        duration: null,
        available: true,
      },
    ]);
    const store = new RecentProjectsStore(new IpcClient(transport));
    expect(store.status).toBe('loading');
    await store.load();
    expect(store.status).toBe('ready');
    expect(store.items.map((p) => p.name)).toEqual(['A']);
  });

  it('keeps the error instead of throwing', async () => {
    const transport = new MemoryTransport().handle('list_recent_projects', () => {
      throw { code: 'io', kind: 'permissionDenied' };
    });
    const store = new RecentProjectsStore(new IpcClient(transport));
    await store.load();
    expect(store.status).toBe('error');
    expect(store.error).toEqual({ code: 'io', kind: 'permissionDenied' });
  });
});
