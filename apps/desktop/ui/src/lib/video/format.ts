// Labels for a video canvas, shared by every screen that shows one.

import type { FrameRate, VideoFormat } from '$lib/ipc/bindings';
import type { Locale } from '$lib/i18n/index.svelte';

function gcd(a: number, b: number): number {
  while (b !== 0) [a, b] = [b, a % b];
  return a;
}

/** `9:16`, `16:9`, `1:1`, `4:5`: the canvas ratio in lowest terms. */
export function aspectLabel(format: Pick<VideoFormat, 'width' | 'height'>): string {
  const g = gcd(format.width, format.height) || 1;
  return `${format.width / g}:${format.height / g}`;
}

/** Frames per second for display: `30`, `29,97` (id) / `29.97` (en). */
export function fpsLabel(rate: FrameRate, locale: Locale): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 3 }).format(rate.num / rate.den);
}
