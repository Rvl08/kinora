import { describe, expect, it, vi } from 'vitest';

import { type KeyInput, ariaShortcut, dispatch, shortcutLabel } from './shortcuts';

function key(
  value: string,
  modifiers: Partial<Omit<KeyInput, 'key' | 'preventDefault'>> = {},
): KeyInput & { prevented: boolean } {
  const input = {
    key: value,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    repeat: false,
    ...modifiers,
    prevented: false,
    preventDefault() {
      input.prevented = true;
    },
  };
  return input;
}

describe('shortcuts', () => {
  it('runs Ctrl+N and Ctrl+O and stops the webview from handling them', () => {
    const newProject = vi.fn();
    const openProject = vi.fn();
    const n = key('n', { ctrlKey: true });
    const o = key('O', { ctrlKey: true }); // Caps Lock
    expect(dispatch(n, { newProject, openProject })).toBe(true);
    expect(dispatch(o, { newProject, openProject })).toBe(true);
    expect(newProject).toHaveBeenCalledOnce();
    expect(openProject).toHaveBeenCalledOnce();
    expect(n.prevented && o.prevented).toBe(true);
  });

  it('ignores other modifiers, plain keys and actions the screen does not offer', () => {
    const newProject = vi.fn();
    for (const input of [
      key('n'),
      key('n', { ctrlKey: true, shiftKey: true }),
      key('n', { ctrlKey: true, altKey: true }),
      key('n', { ctrlKey: true, metaKey: true }),
      key('s', { ctrlKey: true }),
    ]) {
      expect(dispatch(input, { newProject })).toBe(false);
      expect(input.prevented).toBe(false);
    }
    expect(newProject).not.toHaveBeenCalled();
  });

  it('does not repeat an action while the key is held, but still swallows it', () => {
    const saveProject = vi.fn();
    const held = key('s', { ctrlKey: true, repeat: true });
    expect(dispatch(held, { saveProject })).toBe(true);
    expect(held.prevented).toBe(true);
    expect(saveProject).not.toHaveBeenCalled();
  });

  it('leaves keys alone during IME composition', () => {
    const newProject = vi.fn();
    expect(dispatch(key('n', { ctrlKey: true, isComposing: true }), { newProject })).toBe(false);
  });

  it('labels shortcuts for people and for screen readers', () => {
    expect(shortcutLabel('openProject', 'Ctrl')).toBe('Ctrl+O');
    expect(ariaShortcut('newProject')).toBe('Control+N');
    expect(ariaShortcut('saveProject')).toBe('Control+S');
  });
});
