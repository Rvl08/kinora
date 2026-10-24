import { afterEach, describe, expect, it, vi } from 'vitest';

const core = vi.hoisted(() => ({ isTauri: vi.fn<() => boolean>() }));

vi.mock('@tauri-apps/api/core', () => ({ isTauri: core.isTauri, invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { createRuntime } from './runtime';
import { TauriTransport } from './tauri';
import { MemoryTransport } from './transport';

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

describe('createRuntime', () => {
  it('uses Tauri when the official check says the window runs in Tauri', async () => {
    core.isTauri.mockReturnValue(true);
    const runtime = await createRuntime();
    expect(runtime.kind).toBe('tauri');
    expect(runtime.transport).toBeInstanceOf(TauriTransport);
    expect(runtime.platform.watchDrops).toBeTypeOf('function');
  });

  it('uses the demo shell in a development browser tab', async () => {
    core.isTauri.mockReturnValue(false);
    vi.stubEnv('DEV', true);
    vi.stubGlobal('window', { location: { search: '?state=empty' } });
    const runtime = await createRuntime();
    expect(runtime.kind).toBe('demo');
    expect(runtime.platform.watchDrops({ hover() {}, drop() {} })).toBeNull();
  });

  it('has no shell in a production build outside Tauri, and says so', async () => {
    core.isTauri.mockReturnValue(false);
    vi.stubEnv('DEV', false);
    const runtime = await createRuntime();
    expect(runtime.kind).toBe('none');
    expect(runtime.transport).toBeInstanceOf(MemoryTransport);
    await expect(
      runtime.platform.pickFolder({ purpose: 'openProject', title: 'x' }),
    ).rejects.toThrow();
  });
});
