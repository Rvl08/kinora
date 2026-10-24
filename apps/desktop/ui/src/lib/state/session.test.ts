import { describe, expect, it, vi } from 'vitest';

import sample from '../../../../../../crates/kinora-project/tests/fixtures/v0/sample.kinora/project.json';
import type {
  IpcError,
  Project,
  ProjectOpened,
  ProjectSettings,
  ProjectState,
  RecentProject,
} from '$lib/ipc/bindings';
import { IpcClient } from '$lib/ipc/client';
import { MemoryTransport } from '$lib/ipc/transport';
import type { FolderDialogOptions, Platform } from '$lib/platform/platform';

import { Session } from './session.svelte';

const document = sample as unknown as Project;
const settings: ProjectSettings = document.settings;

function state(overrides: Partial<ProjectState> = {}): ProjectState {
  return {
    project: document.id,
    revision: 0,
    dirty: false,
    undoLabel: null,
    redoLabel: null,
    ...overrides,
  };
}

function opened(overrides: Partial<ProjectOpened> = {}, dirty = false): ProjectOpened {
  return {
    snapshot: { state: state({ dirty }), document },
    migratedFrom: null,
    recoveredEdits: 0,
    ...overrides,
  };
}

const missing: RecentProject = {
  path: 'E:\\Flashdisk\\Vlog.kinora',
  name: 'Vlog',
  openedAt: 1,
  format: null,
  duration: null,
  available: false,
};

/** Native dialogs that answer from a queue and remember what they were asked. */
function dialogs(...answers: (string | null | Error)[]) {
  const asked: FolderDialogOptions[] = [];
  const next = async () => {
    const answer = answers.shift() ?? null;
    if (answer instanceof Error) throw answer;
    return answer;
  };
  const platform: Platform = {
    pickFolder: (options) => {
      asked.push(options);
      return next();
    },
    pickVideo: () => next(),
    watchDrops: () => null,
  };
  return { platform, asked };
}

function setup(platform: Platform) {
  const transport = new MemoryTransport()
    .handle('list_recent_projects', () => [missing])
    .handle('open_project', () => opened())
    .handle('create_project', () => opened())
    .handle('forget_recent_project', () => null)
    .handle('get_project', () => ({ state: state(), document }))
    .handle('save_project', () => state())
    .handle('close_project', () => null);
  const session = new Session(new IpcClient(transport), platform);
  const commands = () =>
    transport.calls.filter((c) => c.command !== 'list_recent_projects').map((c) => c.command);
  return { transport, session, commands };
}

describe('Session: start-screen intents', () => {
  it('Open project: folder dialog, then open_project with that folder', async () => {
    const { platform, asked } = dialogs('D:\\Proyek\\A.kinora');
    const { transport, session } = setup(platform);
    await session.openProject();
    expect(asked[0]?.purpose).toBe('openProject');
    expect(transport.calls.at(-1)).toEqual({
      command: 'open_project',
      payload: { args: { path: 'D:\\Proyek\\A.kinora' } },
    });
    expect(session.view).toBe('project');
    expect(session.activity).toBeNull();
  });

  it('a cancelled dialog sends nothing', async () => {
    const { platform } = dialogs(null);
    const { session, commands } = setup(platform);
    await session.openProject();
    expect(commands()).toEqual([]);
    expect(session.view).toBe('start');
    expect(session.notice).toBeNull();
  });

  it('a failing dialog is reported, not thrown', async () => {
    const { platform } = dialogs(new Error('dialog:allow-open missing'));
    const { session, commands } = setup(platform);
    await session.openProject();
    expect(commands()).toEqual([]);
    expect(session.notice?.message).toEqual({ key: 'start.notice.dialogFailed' });
  });

  it('New project: asks for the folder, then create_project with name and settings', async () => {
    const { platform, asked } = dialogs('D:\\Video');
    const { transport, session } = setup(platform);
    const outcome = await session.createProject('Liburan', settings, 'Pantai.mp4');
    expect(outcome).toEqual({ status: 'created' });
    expect(asked[0]?.purpose).toBe('projectLocation');
    expect(transport.calls.at(-1)).toEqual({
      command: 'create_project',
      payload: { args: { parentDir: 'D:\\Video', name: 'Liburan', settings } },
    });
    expect(session.notes?.skippedVideo).toBe('Pantai.mp4');
  });

  it('New project: the next dialog starts in the folder chosen last', async () => {
    const { platform, asked } = dialogs('D:\\Video', 'D:\\Video\\A.kinora');
    const { session } = setup(platform);
    await session.createProject('A', settings);
    await session.requestClose();
    await session.openProject();
    expect(asked[1]?.defaultPath).toBe('D:\\Video');
  });

  it('New project: returns the shell’s refusal to the form instead of a notice', async () => {
    const { platform } = dialogs('D:\\Video');
    const { transport, session } = setup(platform);
    const taken: IpcError = { code: 'io', kind: 'alreadyExists' };
    transport.handle('create_project', () => {
      throw taken;
    });
    expect(await session.createProject('A', settings)).toEqual({ status: 'failed', error: taken });
    expect(session.notice).toBeNull();
    expect(session.view).toBe('start');
  });

  it('New project: cancelling the folder dialog creates nothing', async () => {
    const { platform } = dialogs(null);
    const { session, commands } = setup(platform);
    expect(await session.createProject('A', settings)).toEqual({ status: 'cancelled' });
    expect(commands()).toEqual([]);
  });

  it('Recent: opens the listed path as is', async () => {
    const { platform } = dialogs();
    const { transport, session } = setup(platform);
    await session.openRecent({ ...missing, available: true });
    expect(transport.calls.at(-1)?.payload).toEqual({ args: { path: missing.path } });
  });

  it('Recent: a folder that has gone is reported and the list reloaded', async () => {
    const { platform } = dialogs();
    const { transport, session } = setup(platform);
    transport.handle('open_project', () => {
      throw { code: 'io', kind: 'notFound' };
    });
    await session.openRecent(missing);
    expect(session.notice).toEqual({
      message: { key: 'start.notice.openFailed' },
      error: { code: 'io', kind: 'notFound' },
    });
    expect(transport.calls.at(-1)?.command).toBe('list_recent_projects');
  });

  it('Find folder: opens the new folder, then forgets the old entry', async () => {
    const { platform, asked } = dialogs('D:\\Pindahan\\Vlog.kinora');
    const { transport, session, commands } = setup(platform);
    await session.recent.load();
    await session.locateRecent(missing);
    expect(asked[0]).toMatchObject({ purpose: 'locateProject' });
    expect(asked[0]?.title).toContain('Vlog');
    expect(commands()).toEqual(['open_project', 'forget_recent_project']);
    const [open, forget] = transport.calls.filter((c) => c.command !== 'list_recent_projects');
    expect(open?.payload).toEqual({ args: { path: 'D:\\Pindahan\\Vlog.kinora' } });
    expect(forget?.payload).toEqual({ args: { path: missing.path } });
    expect(session.recent.items).toEqual([]);
    expect(session.view).toBe('project');
  });

  it('Find folder: keeps the entry when the chosen folder does not open', async () => {
    const { platform } = dialogs('D:\\Lain');
    const { transport, session, commands } = setup(platform);
    transport.handle('open_project', () => {
      throw { code: 'projectInvalid', problems: [], omitted: 0 };
    });
    await session.locateRecent(missing);
    expect(commands()).toEqual(['open_project']);
    expect(session.notice?.message).toEqual({ key: 'start.notice.locateFailed' });
  });

  it('Find folder: the same folder is not forgotten once it is back', async () => {
    const { platform } = dialogs(missing.path);
    const { session, commands } = setup(platform);
    await session.locateRecent(missing);
    expect(commands()).toEqual(['open_project']);
  });

  it('Find folder: a failed forget leaves the project open', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const { platform } = dialogs('D:\\Pindahan\\Vlog.kinora');
    const { transport, session } = setup(platform);
    transport.handle('forget_recent_project', () => {
      throw { code: 'internal', incident: 'x' };
    });
    await session.locateRecent(missing);
    expect(session.view).toBe('project');
    expect(session.notice).toBeNull();
    warn.mockRestore();
  });

  it('Remove from list: forget_recent_project with the listed path', async () => {
    const { platform } = dialogs();
    const { transport, session } = setup(platform);
    await session.recent.load();
    await session.forgetRecent(missing);
    expect(transport.calls.at(-1)).toEqual({
      command: 'forget_recent_project',
      payload: { args: { path: missing.path } },
    });
    expect(session.recent.items).toEqual([]);
  });

  it('Remove from list: a refusal keeps the entry and says so', async () => {
    const { platform } = dialogs();
    const { transport, session } = setup(platform);
    await session.recent.load();
    transport.handle('forget_recent_project', () => {
      throw { code: 'io', kind: 'permissionDenied' };
    });
    await session.forgetRecent(missing);
    expect(session.recent.items).toHaveLength(1);
    expect(session.notice?.message).toEqual({ key: 'start.notice.forgetFailed' });
  });

  it('Dropped project folder: open_project with the dropped path', async () => {
    const { platform } = dialogs();
    const { transport, session } = setup(platform);
    await session.openDropped('D:\\Proyek\\B.kinora');
    expect(transport.calls.at(-1)?.payload).toEqual({ args: { path: 'D:\\Proyek\\B.kinora' } });
  });

  it('Choose a video: the file dialog, nothing sent to the shell', async () => {
    const { platform } = dialogs('D:\\Video\\Pantai.mp4');
    const { session, commands } = setup(platform);
    expect(await session.chooseVideo()).toBe('D:\\Video\\Pantai.mp4');
    expect(commands()).toEqual([]);
  });

  it('runs one intent at a time', async () => {
    let answer: (path: string | null) => void = () => {};
    const platform: Platform = {
      pickFolder: () => new Promise((resolve) => (answer = resolve)),
      pickVideo: async () => null,
      watchDrops: () => null,
    };
    const { session, commands } = setup(platform);
    const first = session.openProject();
    expect(session.activity).toBe('choosing');
    await session.openProject(); // ignored while the dialog is open
    await session.openRecent(missing); // ignored too
    answer('D:\\A.kinora');
    await first;
    expect(commands()).toEqual(['open_project']);
  });

  it('keeps what the shell reported about the opened project', async () => {
    const { platform } = dialogs('D:\\A.kinora');
    const { transport, session } = setup(platform);
    transport.handle('open_project', () => opened({ migratedFrom: 0, recoveredEdits: 3 }));
    await session.openProject();
    expect(session.notes).toEqual({ migratedFrom: 0, recoveredEdits: 3, skippedVideo: null });
  });
});

describe('Session: open project', () => {
  async function openWith(dirty: boolean) {
    const { platform } = dialogs();
    const context = setup(platform);
    context.transport.handle('open_project', () => opened({}, dirty));
    await context.session.openRecent(missing);
    return context;
  }

  it('saves in place', async () => {
    const { transport, session } = await openWith(true);
    transport.handle('save_project', () => state({ dirty: false }));
    expect(await session.save()).toBe(true);
    expect(transport.calls.at(-1)).toEqual({
      command: 'save_project',
      payload: { args: { saveAs: null } },
    });
    expect(session.project.state?.dirty).toBe(false);
  });

  it('reports a failed save', async () => {
    const { transport, session } = await openWith(true);
    transport.handle('save_project', () => {
      throw { code: 'io', kind: 'storageFull' };
    });
    expect(await session.save()).toBe(false);
    expect(session.notice).toEqual({
      message: { key: 'workspace.saveFailed' },
      error: { code: 'io', kind: 'storageFull' },
    });
  });

  it('closes a saved project at once and returns to the start screen', async () => {
    const { transport, session } = await openWith(false);
    await session.requestClose();
    expect(transport.calls.at(-2)).toEqual({
      command: 'close_project',
      payload: { args: { discardChanges: false } },
    });
    expect(transport.calls.at(-1)?.command).toBe('list_recent_projects');
    expect(session.view).toBe('start');
  });

  it('asks before closing with unsaved changes; save then close', async () => {
    const { transport, session, commands } = await openWith(true);
    await session.requestClose();
    expect(session.confirmingClose).toBe(true);
    expect(commands()).toEqual(['open_project']);
    await session.saveAndClose();
    expect(commands()).toEqual(['open_project', 'save_project', 'close_project']);
    expect(transport.calls.find((c) => c.command === 'close_project')?.payload).toEqual({
      args: { discardChanges: false },
    });
    expect(session.confirmingClose).toBe(false);
    expect(session.view).toBe('start');
  });

  it('closes without saving only when asked to', async () => {
    const { transport, session } = await openWith(true);
    await session.requestClose();
    await session.discardAndClose();
    expect(transport.calls.find((c) => c.command === 'close_project')?.payload).toEqual({
      args: { discardChanges: true },
    });
    expect(session.view).toBe('start');
  });

  it('cancelling the question keeps the project open', async () => {
    const { session, commands } = await openWith(true);
    await session.requestClose();
    session.cancelClose();
    expect(session.confirmingClose).toBe(false);
    expect(commands()).toEqual(['open_project']);
    expect(session.view).toBe('project');
  });

  it('asks when the shell reports unsaved changes the UI did not know about', async () => {
    const { transport, session } = await openWith(false);
    transport.handle('close_project', () => {
      throw { code: 'unsavedChanges' };
    });
    await session.requestClose();
    expect(session.confirmingClose).toBe(true);
    expect(session.view).toBe('project');
  });

  it('a save that fails during save-and-close keeps the project open', async () => {
    const { transport, session, commands } = await openWith(true);
    transport.handle('save_project', () => {
      throw { code: 'io', kind: 'busy' };
    });
    await session.requestClose();
    await session.saveAndClose();
    expect(commands()).toEqual(['open_project', 'save_project']);
    expect(session.view).toBe('project');
    expect(session.notice?.message).toEqual({ key: 'workspace.saveFailed' });
  });

  it('follows a close from elsewhere (the local API)', async () => {
    const { platform } = dialogs();
    const { transport, session } = setup(platform);
    const stop = await session.start();
    await session.openRecent(missing);
    transport.emit('project:changed', { reason: 'closed', state: null });
    expect(session.view).toBe('start');
    expect(session.notes).toBeNull();
    stop();
  });
});
