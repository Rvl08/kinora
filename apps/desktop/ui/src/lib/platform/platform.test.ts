import { beforeEach, describe, expect, it, vi } from 'vitest';

type DragDrop =
  | { type: 'enter'; paths: string[] }
  | { type: 'over' }
  | { type: 'drop'; paths: string[] }
  | { type: 'leave' };

const tauri = vi.hoisted(() => ({
  open: vi.fn<(options: unknown) => Promise<string | string[] | null>>(),
  onDragDropEvent: vi.fn<(handler: (event: { payload: unknown }) => void) => Promise<() => void>>(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: tauri.open }));
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ onDragDropEvent: tauri.onDragDropEvent }),
}));

import { baseName, classifyDrop, droppedPath } from './drop';
import { createTauriPlatform } from './tauri';

beforeEach(() => {
  tauri.open.mockReset();
  tauri.onDragDropEvent.mockReset();
});

describe('Tauri platform', () => {
  it('asks for one folder with the given title and start folder', async () => {
    tauri.open.mockResolvedValue('D:\\Video');
    const path = await createTauriPlatform().pickFolder({
      purpose: 'projectLocation',
      title: 'Pilih folder',
      defaultPath: 'D:\\',
    });
    expect(path).toBe('D:\\Video');
    expect(tauri.open).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      title: 'Pilih folder',
      defaultPath: 'D:\\',
    });
  });

  it('reports a cancelled dialog as null', async () => {
    tauri.open.mockResolvedValue(null);
    expect(await createTauriPlatform().pickFolder({ purpose: 'openProject', title: 'x' })).toBe(
      null,
    );
    expect(tauri.open.mock.calls[0]?.[0]).not.toHaveProperty('defaultPath');
  });

  it('filters the video dialog to video containers', async () => {
    tauri.open.mockResolvedValue(['D:\\a.mp4']);
    const path = await createTauriPlatform().pickVideo({ title: 'Video', filterName: 'Video' });
    expect(path).toBe('D:\\a.mp4');
    const options = tauri.open.mock.calls[0]?.[0] as { filters: { extensions: string[] }[] };
    expect(options.filters[0]?.extensions).toContain('mp4');
    expect(options.filters[0]?.extensions).toContain('mov');
  });

  it('turns webview drag and drop into hover and drop', async () => {
    const stop = vi.fn();
    let emit: ((event: { payload: DragDrop }) => void) | undefined;
    tauri.onDragDropEvent.mockImplementation(async (handler) => {
      emit = handler;
      return stop;
    });
    const hover: boolean[] = [];
    const drops: (readonly string[])[] = [];
    const watching = createTauriPlatform().watchDrops({
      hover: (active) => hover.push(active),
      drop: (paths) => drops.push(paths),
    });
    const unlisten = await watching;
    emit?.({ payload: { type: 'enter', paths: ['D:\\a.mp4'] } });
    emit?.({ payload: { type: 'over' } });
    emit?.({ payload: { type: 'leave' } });
    emit?.({ payload: { type: 'drop', paths: ['D:\\a.mp4'] } });
    emit?.({ payload: { type: 'drop', paths: [] } });
    expect(hover).toEqual([true, true, false, false, false]);
    expect(drops).toEqual([['D:\\a.mp4']]);
    unlisten?.();
    unlisten?.();
    expect(stop).toHaveBeenCalledTimes(1);
  });
});

describe('drops', () => {
  it('reads the last path component only', () => {
    expect(baseName('D:\\Video\\Pantai.mp4')).toBe('Pantai.mp4');
    expect(baseName('D:\\Proyek\\Liburan.kinora\\')).toBe('Liburan.kinora');
    expect(baseName('/home/a/b.mov')).toBe('b.mov');
    expect(baseName('lone.mp4')).toBe('lone.mp4');
  });

  it('opens a dropped project folder before anything else', () => {
    const items = [droppedPath('D:\\a.mp4'), droppedPath('D:\\Proyek\\Liburan.KINORA')];
    expect(classifyDrop(items)).toEqual({ kind: 'project', path: 'D:\\Proyek\\Liburan.KINORA' });
  });

  it('starts a project for the first video, keeping the path untouched', () => {
    expect(classifyDrop([droppedPath('D:\\foto.jpg'), droppedPath('D:\\Pantai.MP4')])).toEqual({
      kind: 'video',
      name: 'Pantai.MP4',
    });
  });

  it('cannot open a browser drop as a project (no path)', () => {
    expect(classifyDrop([{ name: 'A.kinora', path: null }])).toEqual({
      kind: 'unsupported',
      name: 'A.kinora',
    });
  });

  it('refuses other files and ignores an empty drop', () => {
    expect(classifyDrop([droppedPath('D:\\catatan.txt')])).toEqual({
      kind: 'unsupported',
      name: 'catatan.txt',
    });
    expect(classifyDrop([])).toBeNull();
  });
});
