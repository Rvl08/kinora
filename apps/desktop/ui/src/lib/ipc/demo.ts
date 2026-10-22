// Development only: a stand-in shell for previewing the UI in a browser,
// where Tauri is absent. Imported behind `import.meta.env.DEV`, so it never
// reaches a production build. Every value here is synthetic and labelled as
// an example.

import type { RecentProject } from './bindings';
import { MemoryTransport } from './transport';

const HOUR = 3_600_000;

function sample(now: number): RecentProject[] {
  const vertical = { width: 1080, height: 1920, frameRate: { num: 30, den: 1 } };
  const landscape = { width: 1920, height: 1080, frameRate: { num: 30, den: 1 } };
  const square = { width: 1080, height: 1080, frameRate: { num: 30, den: 1 } };
  return [
    {
      path: 'D:\\Contoh\\Liburan di Bali.kinora',
      name: 'Liburan di Bali (contoh)',
      openedAt: now - 2 * HOUR,
      format: vertical,
      duration: { num: 58, den: 1 },
      available: true,
    },
    {
      path: 'D:\\Contoh\\Resep sambal matah.kinora',
      name: 'Resep sambal matah (contoh)',
      openedAt: now - 26 * HOUR,
      format: vertical,
      duration: { num: 47, den: 2 },
      available: true,
    },
    {
      path: 'D:\\Contoh\\Review laptop kerja.kinora',
      name: 'Review laptop kerja (contoh)',
      openedAt: now - 5 * 24 * HOUR,
      format: landscape,
      duration: { num: 734, den: 1 },
      available: true,
    },
    {
      path: 'D:\\Contoh\\Promo toko kue.kinora',
      name: 'Promo toko kue (contoh)',
      openedAt: now - 9 * 24 * HOUR,
      format: square,
      duration: { num: 30, den: 1 },
      available: true,
    },
    {
      path: 'E:\\Flashdisk\\Vlog wisuda.kinora',
      name: 'Vlog wisuda (contoh)',
      openedAt: now - 40 * 24 * HOUR,
      format: null,
      duration: null,
      available: false,
    },
  ];
}

/** `?state=empty|error|loading` in the URL previews the other start-screen states. */
export function createDemoTransport(search: string, now: number): MemoryTransport {
  const state = new URLSearchParams(search).get('state');
  return new MemoryTransport().handle('list_recent_projects', () => {
    if (state === 'error') throw { code: 'io', kind: 'permissionDenied' };
    if (state === 'loading') return new Promise(() => {});
    return state === 'empty' ? [] : sample(now);
  });
}
