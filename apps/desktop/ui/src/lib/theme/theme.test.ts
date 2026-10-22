import { describe, expect, it } from 'vitest';

import { ThemeController, resolveTheme } from './theme.svelte';

describe('theme', () => {
  it('follows the system unless the user chose a theme', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
  });

  it('works without a DOM (tests, prerendering)', () => {
    const controller = new ThemeController('dark');
    controller.start()();
    controller.preference = 'light';
    expect(controller.theme).toBe('light');
  });
});
