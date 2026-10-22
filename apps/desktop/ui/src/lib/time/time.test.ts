import { describe, expect, it } from 'vitest';

import {
  formatDuration,
  formatTimecode,
  frameAt,
  frameStart,
  isOnGrid,
  rational,
  snapToFrame,
  toSeconds,
} from './time';

const ntsc = { num: 30_000, den: 1001 };
const fps25 = { num: 25, den: 1 };

describe('rational', () => {
  it('reduces and normalises the sign', () => {
    expect(rational(2n, 4n)).toEqual({ num: 1, den: 2 });
    expect(rational(3n, -6n)).toEqual({ num: -1, den: 2 });
  });

  it('refuses values the wire format cannot carry', () => {
    expect(() => rational(1n, 0n)).toThrow(RangeError);
    expect(() => rational(2n ** 53n, 1n)).toThrow(RangeError);
    expect(() => rational(1n, 2n ** 32n)).toThrow(RangeError);
  });
});

describe('frames', () => {
  it('find the frame shown at a time, with boundaries belonging to the later frame', () => {
    expect(frameAt({ num: 1, den: 25 }, fps25)).toBe(1);
    expect(frameAt({ num: 1, den: 26 }, fps25)).toBe(0);
    expect(frameAt({ num: -1, den: 50 }, fps25)).toBe(-1);
  });

  it('are exact at NTSC rates over an hour', () => {
    expect(frameAt({ num: 3600, den: 1 }, ntsc)).toBe(107_892);
    expect(frameStart(107_892, ntsc)).toEqual(rational(107_892n * 1001n, 30_000n));
    expect(frameStart(107_892, ntsc)).toEqual({ num: 8_999_991, den: 2500 });
  });

  it('snap to the start of the frame and stay on the edit grid', () => {
    const snapped = snapToFrame({ num: 7, den: 100 }, fps25);
    expect(snapped).toEqual({ num: 1, den: 25 });
    expect(isOnGrid(snapped)).toBe(true);
    expect(isOnGrid(frameStart(12_345, ntsc))).toBe(true);
    expect(isOnGrid({ num: 1, den: 1001 })).toBe(false);
  });
});

describe('formatting', () => {
  it('writes non-drop-frame timecode', () => {
    expect(formatTimecode({ num: 0, den: 1 }, fps25)).toBe('00:00:00:00');
    expect(formatTimecode({ num: 3725, den: 1 }, fps25)).toBe('01:02:05:00');
    expect(formatTimecode({ num: 49, den: 25 }, fps25)).toBe('00:00:01:24');
    expect(formatTimecode(frameStart(30, ntsc), ntsc)).toBe('00:00:01:00');
  });

  it('writes durations for people', () => {
    expect(formatDuration({ num: 5, den: 1 })).toBe('0:05');
    expect(formatDuration({ num: 131, den: 2 })).toBe('1:05');
    expect(formatDuration({ num: 3725, den: 1 })).toBe('1:02:05');
    expect(toSeconds({ num: 1, den: 4 })).toBe(0.25);
  });
});
