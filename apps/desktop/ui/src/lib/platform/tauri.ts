// Native dialogs (tauri-plugin-dialog) and the webview's drag-and-drop
// events, inside the Tauri shell. Loaded only when the window runs in Tauri.

import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';

import type { Unlisten } from '$lib/ipc/transport';

import { type DropHandlers, type Platform, VIDEO_EXTENSIONS } from './platform';

/** `open` returns an array only for `multiple: true`; guard anyway. */
function single(result: string | string[] | null): string | null {
  if (Array.isArray(result)) return result[0] ?? null;
  return result;
}

async function watchDrops(handlers: DropHandlers): Promise<Unlisten> {
  const stop = await getCurrentWebview().onDragDropEvent(({ payload }) => {
    switch (payload.type) {
      case 'enter':
      case 'over':
        handlers.hover(true);
        break;
      case 'leave':
        handlers.hover(false);
        break;
      case 'drop':
        handlers.hover(false);
        if (payload.paths.length > 0) handlers.drop(payload.paths);
        break;
    }
  });
  let active = true;
  return () => {
    if (!active) return;
    active = false;
    try {
      stop();
    } catch (reason) {
      console.warn('Kinora: could not stop listening to drops', reason);
    }
  };
}

export function createTauriPlatform(): Platform {
  return {
    async pickFolder({ title, defaultPath }) {
      return single(
        await open({
          directory: true,
          multiple: false,
          title,
          ...(defaultPath && { defaultPath }),
        }),
      );
    },
    async pickVideo({ title, defaultPath, filterName }) {
      return single(
        await open({
          directory: false,
          multiple: false,
          title,
          filters: [{ name: filterName, extensions: [...VIDEO_EXTENSIONS] }],
          ...(defaultPath && { defaultPath }),
        }),
      );
    },
    watchDrops,
  };
}
