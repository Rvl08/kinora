// Ratio presets for new projects and aspect labels for existing ones.

import type { ProjectSettings, VideoFormat } from '$lib/ipc/bindings';

export type RatioPreset = 'vertical' | 'landscape' | 'square';

/** Presets in the order the start screen offers them; vertical leads. */
export const RATIO_PRESETS: readonly RatioPreset[] = ['vertical', 'landscape', 'square'];

const FORMATS: Readonly<Record<RatioPreset, VideoFormat>> = {
  vertical: { width: 1080, height: 1920, frameRate: { num: 30, den: 1 } },
  landscape: { width: 1920, height: 1080, frameRate: { num: 30, den: 1 } },
  square: { width: 1080, height: 1080, frameRate: { num: 30, den: 1 } },
};

/** Settings for a new project with this preset: 1080p, 30 fps, Rec. 709, 48 kHz stereo. */
export function settingsFor(preset: RatioPreset): ProjectSettings {
  return {
    defaultFormat: FORMATS[preset],
    colorSpace: 'rec709',
    audio: { sampleRate: 48_000, channels: 'stereo' },
  };
}

function gcd(a: number, b: number): number {
  while (b !== 0) [a, b] = [b, a % b];
  return a;
}

/** `9:16`, `16:9`, `1:1`, `4:5`: the canvas ratio in lowest terms. */
export function aspectLabel(format: Pick<VideoFormat, 'width' | 'height'>): string {
  const g = gcd(format.width, format.height) || 1;
  return `${format.width / g}:${format.height / g}`;
}

/** The ratio label of a preset. */
export function presetLabel(preset: RatioPreset): string {
  return aspectLabel(FORMATS[preset]);
}
