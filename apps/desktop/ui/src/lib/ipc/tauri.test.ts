import { beforeEach, describe, expect, it, vi } from 'vitest';

const tauri = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: unknown) => Promise<unknown>>(),
  listen:
    vi.fn<
      (event: string, handler: (message: { payload: unknown }) => void) => Promise<() => void>
    >(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: tauri.listen }));

import type { IpcError, ProjectChanged } from './bindings';
import { IpcClient } from './client';
import { IpcFailure } from './errors';
import { TauriTransport } from './tauri';

beforeEach(() => {
  tauri.invoke.mockReset();
  tauri.listen.mockReset();
});

describe('TauriTransport', () => {
  it('invokes commands with `{ args }`, or no arguments', async () => {
    tauri.invoke.mockResolvedValue(null);
    const client = new IpcClient(new TauriTransport());
    await client.call('open_project', { path: 'D:\\A.kinora' });
    await client.call('list_recent_projects', null);
    await client.call('forget_recent_project', { path: 'D:\\A.kinora' });
    expect(tauri.invoke.mock.calls).toEqual([
      ['open_project', { args: { path: 'D:\\A.kinora' } }],
      ['list_recent_projects', {}],
      ['forget_recent_project', { args: { path: 'D:\\A.kinora' } }],
    ]);
  });

  it('returns the command output', async () => {
    tauri.invoke.mockResolvedValue([]);
    expect(await new IpcClient(new TauriTransport()).call('list_recent_projects', null)).toEqual(
      [],
    );
  });

  it('passes the shell error through as an IpcFailure', async () => {
    const error: IpcError = { code: 'io', kind: 'permissionDenied' };
    tauri.invoke.mockRejectedValue(error);
    const failure = await new IpcClient(new TauriTransport())
      .call('open_project', { path: 'x' })
      .catch((e: unknown) => e);
    expect(failure).toBeInstanceOf(IpcFailure);
    expect((failure as IpcFailure).error).toEqual(error);
  });

  it('turns a refusal by Tauri itself (a string) into an internal error', async () => {
    tauri.invoke.mockRejectedValue('Command forget_recent_project not found');
    const failure = (await new IpcClient(new TauriTransport())
      .call('forget_recent_project', { path: 'x' })
      .catch((e: unknown) => e)) as IpcFailure;
    expect(failure.error).toEqual({ code: 'internal', incident: '' });
    expect(failure.cause).toBe('Command forget_recent_project not found');
  });

  it('delivers event payloads and stops listening once', async () => {
    const stop = vi.fn();
    let deliver: ((message: { payload: unknown }) => void) | undefined;
    tauri.listen.mockImplementation(async (_event, handler) => {
      deliver = handler;
      return stop;
    });
    const seen: ProjectChanged[] = [];
    const unlisten = await new IpcClient(new TauriTransport()).on('project:changed', (event) =>
      seen.push(event),
    );
    expect(tauri.listen).toHaveBeenCalledWith('project:changed', expect.any(Function));
    deliver?.({ payload: { reason: 'saved', state: null } });
    expect(seen).toEqual([{ reason: 'saved', state: null }]);
    unlisten();
    unlisten();
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it('survives a webview that is already gone when unlistening', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    tauri.listen.mockResolvedValue(() => {
      throw new Error('webview destroyed');
    });
    const unlisten = await new TauriTransport().listen('job:progress', () => {});
    expect(() => unlisten()).not.toThrow();
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it('reports a refused subscription as an IpcFailure', async () => {
    tauri.listen.mockRejectedValue('event.listen not allowed');
    await expect(
      new IpcClient(new TauriTransport()).on('project:changed', () => {}),
    ).rejects.toBeInstanceOf(IpcFailure);
  });
});
