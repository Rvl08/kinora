// Picks how this window reaches the shell.

import { MemoryTransport, type Transport } from './transport';

/**
 * The transport for the running app. In development without Tauri (a plain
 * browser tab) the demo shell is used. The Tauri transport arrives with the
 * shell (apps/desktop/src-tauri, Phase 1); until then a production build
 * reports every call as an internal error, which the UI shows honestly.
 */
export async function createTransport(): Promise<Transport> {
  if (import.meta.env.DEV) {
    const { createDemoTransport } = await import('./demo');
    return createDemoTransport(window.location.search, Date.now());
  }
  return new MemoryTransport();
}
