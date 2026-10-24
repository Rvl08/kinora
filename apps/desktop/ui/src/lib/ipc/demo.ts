// Development only: a stand-in shell for previewing the UI in a browser,
// where Tauri is absent. Imported behind `import.meta.env.DEV`, so it never
// reaches a production build. Every value here is synthetic and labelled as
// an example, and its "dialogs" answer with fixed example paths.

import type { Platform } from '$lib/platform/platform';

import type {
  Project,
  ProjectOpened,
  ProjectSettings,
  ProjectState,
  RecentProject,
} from './bindings';
import { MemoryTransport } from './transport';

const HOUR = 3_600_000;
const FOLDER = 'D:\\Contoh';

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

const DEFAULT_SETTINGS: ProjectSettings = {
  defaultFormat: { width: 1080, height: 1920, frameRate: { num: 30, den: 1 } },
  colorSpace: 'rec709',
  audio: { sampleRate: 48_000, channels: 'stereo' },
};

/** An empty project document, as the shell would create it. */
function document(name: string, settings: ProjectSettings): Project {
  const id = crypto.randomUUID();
  return {
    schemaVersion: 0,
    id,
    name,
    settings,
    media: [],
    timelines: [
      {
        id: crypto.randomUUID(),
        name: 'Timeline 1',
        format: settings.defaultFormat,
        tracks: [],
        markers: [],
      },
    ],
  };
}

/** The demo shell's one open project, if any. */
interface Open {
  document: Project;
  state: ProjectState;
}

/**
 * `?state=empty|error|loading` previews the other start-screen states;
 * `?state=fail` makes opening and creating fail (as a full disk would);
 * `?state=recovered` opens projects as after a crash (unsaved, recovered
 * edits, migrated), to preview those notes and the close question.
 */
export function createDemoRuntime(
  search: string,
  now: number,
): { transport: MemoryTransport; platform: Platform } {
  const state = new URLSearchParams(search).get('state');
  let recent = state === 'empty' ? [] : sample(now);
  let open: Open | null = null;

  const recovered = state === 'recovered';
  const opened = (doc: Project, path: string): ProjectOpened => {
    open = {
      document: doc,
      state: { project: doc.id, revision: 0, dirty: recovered, undoLabel: null, redoLabel: null },
    };
    recent = [
      {
        path,
        name: doc.name,
        openedAt: Date.now(),
        format: doc.settings.defaultFormat,
        duration: { num: 0, den: 1 },
        available: true,
      },
      ...recent.filter((item) => item.path !== path),
    ];
    return {
      snapshot: { ...open },
      migratedFrom: recovered ? 0 : null,
      recoveredEdits: recovered ? 3 : 0,
    };
  };

  const transport = new MemoryTransport()
    .handle('list_recent_projects', () => {
      if (state === 'error') throw { code: 'io', kind: 'permissionDenied' };
      if (state === 'loading') return new Promise(() => {});
      return recent;
    })
    .handle('forget_recent_project', ({ args }) => {
      const { path } = args as { path: string };
      recent = recent.filter((item) => item.path !== path);
      return null;
    })
    .handle('create_project', ({ args }) => {
      if (state === 'fail') throw { code: 'io', kind: 'storageFull' };
      const { parentDir, name, settings } = args as {
        parentDir: string;
        name: string;
        settings: ProjectSettings;
      };
      // The stand-in shell names the folder, as the real one does; the UI never does.
      const path = `${parentDir}\\${name}.kinora`;
      if (recent.some((item) => item.path === path)) throw { code: 'io', kind: 'alreadyExists' };
      return opened(document(name, settings), path);
    })
    .handle('open_project', ({ args }) => {
      if (state === 'fail') throw { code: 'io', kind: 'permissionDenied' };
      const { path } = args as { path: string };
      const entry = recent.find((item) => item.path === path);
      if (!entry?.available) throw { code: 'io', kind: 'notFound' };
      return opened(
        document(entry.name, {
          ...DEFAULT_SETTINGS,
          defaultFormat: entry.format ?? DEFAULT_SETTINGS.defaultFormat,
        }),
        path,
      );
    })
    .handle('get_project', () => {
      if (!open) throw { code: 'noProject' };
      return { ...open };
    })
    .handle('save_project', () => {
      if (!open) throw { code: 'noProject' };
      open = { ...open, state: { ...open.state, dirty: false } };
      return open.state;
    })
    .handle('close_project', ({ args }) => {
      const { discardChanges } = args as { discardChanges: boolean };
      if (open?.state.dirty && !discardChanges) throw { code: 'unsavedChanges' };
      open = null;
      return null;
    });

  const platform: Platform = {
    // "Find folder…" and "Open project…" pick the first example project.
    pickFolder: async ({ purpose }) =>
      purpose === 'projectLocation'
        ? FOLDER
        : (recent.find((item) => item.available)?.path ?? null),
    pickVideo: async () => `${FOLDER}\\Klip pantai (contoh).mp4`,
    watchDrops: () => null,
  };

  return { transport, platform };
}
