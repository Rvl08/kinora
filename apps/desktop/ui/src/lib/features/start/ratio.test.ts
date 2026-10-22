import { describe, expect, it } from 'vitest';

import { describeDate } from '$lib/i18n/format';

import { aspectLabel, presetLabel, settingsFor } from './ratio';

describe('ratios', () => {
  it('labels canvases in lowest terms', () => {
    expect(aspectLabel({ width: 1080, height: 1920 })).toBe('9:16');
    expect(aspectLabel({ width: 3840, height: 2160 })).toBe('16:9');
    expect(aspectLabel({ width: 1080, height: 1350 })).toBe('4:5');
    expect(presetLabel('square')).toBe('1:1');
  });

  it('builds settings on a grid-aligned frame rate', () => {
    const settings = settingsFor('vertical');
    expect(settings.defaultFormat).toMatchObject({ width: 1080, height: 1920 });
    expect(settings.defaultFormat.frameRate).toEqual({ num: 30, den: 1 });
  });
});

describe('describeDate', () => {
  const now = new Date(2026, 9, 22, 15, 0).getTime();

  it('names today and yesterday', () => {
    expect(describeDate(new Date(2026, 9, 22, 9, 5).getTime(), now, 'id')).toMatchObject({
      key: 'time.today',
    });
    expect(describeDate(new Date(2026, 9, 21, 23, 59).getTime(), now, 'en')).toMatchObject({
      key: 'time.yesterday',
    });
  });

  it('writes older dates, with the year only when it differs', () => {
    expect(describeDate(new Date(2026, 9, 12).getTime(), now, 'en')).toBe('Oct 12');
    expect(describeDate(new Date(2025, 9, 12).getTime(), now, 'id')).toBe('12 Okt 2025');
  });
});
