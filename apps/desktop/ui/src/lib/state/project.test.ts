import { describe, expect, it } from 'vitest';

import sample from '../../../../../../crates/kinora-project/tests/fixtures/v0/sample.kinora/project.json';
import type { IpcError, Project, ProjectSnapshot, ProjectState } from '$lib/ipc/bindings';
import { IpcClient } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/errors';
import { MemoryTransport } from '$lib/ipc/transport';

import { actionLabel } from './history';
import { ProjectStore } from './project.svelte';

const document = sample as unknown as Project;

function state(revision: number, overrides: Partial<ProjectState> = {}): ProjectState {
  return {
    project: document.id,
    revision,
    dirty: revision > 0,
    undoLabel: null,
    redoLabel: null,
    ...overrides,
  };
}

/** A shell whose revision the test controls. */
function shell() {
  let revision = 0;
  const transport = new MemoryTransport();
  const snapshot = (): ProjectSnapshot => ({ state: state(revision), document });
  transport.handle('get_project', () => snapshot());
  const store = new ProjectStore(new IpcClient(transport));
  store.opened({ snapshot: snapshot(), migratedFrom: null, recoveredEdits: 0 });
  const fetches = () => transport.calls.filter((c) => c.command === 'get_project').length;
  return {
    transport,
    store,
    fetches,
    advance: () => ++revision,
  };
}

describe('ProjectStore', () => {
  it('sends edits against the shown revision and pulls the new document', async () => {
    const { transport, store, advance, fetches } = shell();
    transport.handle('apply_command', (payload) => {
      expect(payload).toMatchObject({ args: { baseRevision: 0 } });
      return state(advance(), { undoLabel: 'renameClip' });
    });
    await store.apply({ op: 'renameClip', params: { clip: 'x', name: null } });
    expect(store.state?.revision).toBe(1);
    expect(fetches()).toBe(1);
  });

  it('fetches once for an edit and its change event', async () => {
    const { transport, store, advance, fetches } = shell();
    transport.handle('undo', () => {
      const next = state(advance());
      transport.emit('project:changed', { reason: 'undone', state: next });
      return next;
    });
    await store.connect();
    await store.undo();
    expect(fetches()).toBe(1);
  });

  it('updates the state without fetching when the document did not change', async () => {
    const { transport, store, fetches } = shell();
    await store.connect();
    transport.emit('project:changed', { reason: 'saved', state: state(0, { dirty: false }) });
    expect(store.state?.dirty).toBe(false);
    expect(fetches()).toBe(0);
  });

  it('catches up after a revision conflict and still reports it', async () => {
    const { transport, store, advance } = shell();
    advance(); // another window edited
    const conflict: IpcError = { code: 'revisionConflict', expected: 0, current: 1 };
    transport.handle('redo', () => {
      throw conflict;
    });
    const failure = await store.redo().catch((e: unknown) => e);
    expect(failure).toBeInstanceOf(IpcFailure);
    expect(store.state?.revision).toBe(1);
  });

  it('forgets the document when the project closes', async () => {
    const { transport, store } = shell();
    await store.connect();
    transport.emit('project:changed', { reason: 'closed', state: null });
    expect(store.document).toBeNull();
    await expect(store.undo()).rejects.toBeInstanceOf(IpcFailure);
  });
});

describe('actionLabel', () => {
  it('translates operation names and keeps batch labels', () => {
    expect(actionLabel('splitClip')).toEqual({ key: 'ops.splitClip' });
    expect(actionLabel('Hapus kata pengisi')).toBe('Hapus kata pengisi');
    expect(actionLabel('toString')).toBe('toString');
  });
});
