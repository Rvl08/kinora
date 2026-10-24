// Picks how this window reaches the shell and the desktop.

import { isTauri } from '@tauri-apps/api/core';

import { type Platform, UNAVAILABLE } from '$lib/platform/platform';

import { MemoryTransport, type Transport } from './transport';

export interface Runtime {
  /** `tauri` inside the shell, `demo` in a development browser tab, `none` otherwise. */
  readonly kind: 'tauri' | 'demo' | 'none';
  readonly transport: Transport;
  readonly platform: Platform;
}

/**
 * Inside the Tauri shell: the Tauri transport, native dialogs and native
 * drag and drop. In development without Tauri (a plain browser tab): the
 * demo shell. A production build opened outside the shell has neither, so
 * every call fails and the UI says so.
 */
export async function createRuntime(): Promise<Runtime> {
  if (isTauri()) {
    const [{ TauriTransport }, { createTauriPlatform }] = await Promise.all([
      import('./tauri'),
      import('$lib/platform/tauri'),
    ]);
    return { kind: 'tauri', transport: new TauriTransport(), platform: createTauriPlatform() };
  }
  if (import.meta.env.DEV) {
    const { createDemoRuntime } = await import('./demo');
    return { kind: 'demo', ...createDemoRuntime(window.location.search, Date.now()) };
  }
  return { kind: 'none', transport: new MemoryTransport(), platform: UNAVAILABLE };
}
