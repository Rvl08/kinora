import { describe, expect, it } from 'vitest';

import { I18n } from '$lib/i18n/index.svelte';

import type { IpcError } from './bindings';
import { IpcClient } from './client';
import { IpcFailure, describeError, isIpcError, toFailure } from './errors';
import { MemoryTransport } from './transport';

describe('IpcClient', () => {
  it('sends `{ args }`, or nothing for commands without arguments', async () => {
    const transport = new MemoryTransport()
      .handle('undo', () => null)
      .handle('get_project', () => null);
    const client = new IpcClient(transport);
    await client.call('undo', { baseRevision: 4 });
    await client.call('get_project', null);
    expect(transport.calls).toEqual([
      { command: 'undo', payload: { args: { baseRevision: 4 } } },
      { command: 'get_project', payload: {} },
    ]);
  });

  it('rejects with the shell error as an IpcFailure', async () => {
    const error: IpcError = { code: 'nothingToUndo' };
    const transport = new MemoryTransport().handle('undo', () => {
      throw error;
    });
    const failure = await new IpcClient(transport)
      .call('undo', { baseRevision: 0 })
      .catch((e: unknown) => e);
    expect(failure).toBeInstanceOf(IpcFailure);
    expect((failure as IpcFailure).error).toEqual(error);
  });

  it('delivers typed events until unsubscribed', async () => {
    const transport = new MemoryTransport();
    const seen: string[] = [];
    const stop = await new IpcClient(transport).on('project:changed', (event) => {
      seen.push(event.reason);
    });
    transport.emit('project:changed', { reason: 'saved', state: null });
    stop();
    transport.emit('project:changed', { reason: 'closed', state: null });
    expect(seen).toEqual(['saved']);
  });
});

describe('errors', () => {
  it('recognises contract errors only', () => {
    expect(isIpcError({ code: 'cancelled' })).toBe(true);
    expect(isIpcError({ code: 'explode' })).toBe(false);
    expect(isIpcError('cancelled')).toBe(false);
    expect(isIpcError(null)).toBe(false);
  });

  it('turns transport failures into internal errors that keep the cause', () => {
    const cause = new TypeError('ipc unavailable');
    const failure = toFailure(cause);
    expect(failure.error).toEqual({ code: 'internal', incident: '' });
    expect(failure.cause).toBe(cause);
  });

  it('describes every error in both languages', () => {
    const errors: IpcError[] = [
      { code: 'noProject' },
      { code: 'notFound', entity: 'clip', id: 'x' },
      { code: 'editRejected', rule: 'overlap', reason: 'x', command: '' },
      { code: 'projectTooNew', found: 3, supported: 0 },
      { code: 'io', kind: 'storageFull' },
      { code: 'internal', incident: '0192f1c4' },
      { code: 'projectInvalid', problems: [], omitted: 0 },
    ];
    for (const locale of ['id', 'en'] as const) {
      const i18n = new I18n(locale);
      for (const error of errors) {
        const text = i18n.message(describeError(error));
        expect(text).not.toMatch(/^errors\./);
        expect(text).not.toContain('{');
      }
    }
    expect(new I18n('en').message(describeError({ code: 'internal', incident: 'abc' }))).toContain(
      'abc',
    );
    expect(
      new I18n('id').message(describeError({ code: 'notFound', entity: 'clip', id: 'x' })),
    ).toBe('Klip tidak ditemukan. Mungkin sudah dihapus.');
  });
});
