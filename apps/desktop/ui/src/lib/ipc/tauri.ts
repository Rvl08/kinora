// The transport inside the Tauri shell: `invoke` for commands and the event
// system for `project:changed` and `job:progress`. Loaded only when the
// window runs in Tauri (runtime.ts).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { Transport, Unlisten } from './transport';

/**
 * Commands reject with the shell's `IpcError` value, or with a string when
 * Tauri itself refuses the call (unknown command, missing permission, bad
 * arguments). The client turns both into `IpcFailure` (errors.ts).
 */
export class TauriTransport implements Transport {
  invoke(command: string, payload: Record<string, unknown>): Promise<unknown> {
    return invoke<unknown>(command, payload);
  }

  async listen(event: string, handler: (payload: unknown) => void): Promise<Unlisten> {
    const stop = await listen<unknown>(event, (message) => handler(message.payload));
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      try {
        stop();
      } catch (reason) {
        // The webview may already be gone (window closing); its listeners
        // are dropped with it, so there is nothing left to clean up.
        console.warn(`Kinora: could not stop listening to ${event}`, reason);
      }
    };
  }
}
