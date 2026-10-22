// Exact time at the edge of the UI (ADR-0004, amendment 2).
//
// The document stores time as rationals `{ num, den }` on a grid of
// 705,600,000 flicks per second. The UI converts to seconds or timecode only
// for display, and builds the rationals it sends back from whole frames, so
// every time it sends lies on the grid. Arithmetic uses BigInt: products of
// numerators and frame rates exceed 2^53.

import type { FrameRate, Rational, Time } from '$lib/ipc/bindings';

/** Ticks per second of the edit grid. */
export const FLICKS_PER_SECOND = 705_600_000;

function gcd(a: bigint, b: bigint): bigint {
  let [x, y] = [a < 0n ? -a : a, b < 0n ? -b : b];
  while (y !== 0n) [x, y] = [y, x % y];
  return x === 0n ? 1n : x;
}

/** `num / den` in lowest terms; throws when it does not fit the wire format. */
export function rational(num: bigint, den: bigint): Rational {
  if (den === 0n) throw new RangeError('denominator is zero');
  const sign = den < 0n ? -1n : 1n;
  const g = gcd(num, den);
  const n = (sign * num) / g;
  const d = (sign * den) / g;
  if (n > BigInt(Number.MAX_SAFE_INTEGER) || n < -BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new RangeError('numerator exceeds 2^53 - 1');
  }
  if (d > 0xffff_ffffn) throw new RangeError('denominator exceeds u32');
  return { num: Number(n), den: Number(d) };
}

/** Seconds as a float. Lossy: for drawing and display only, never sent back. */
export function toSeconds(time: Time): number {
  return time.num / time.den;
}

/** The index of the frame shown at `time`: `floor(time × rate)`. */
export function frameAt(time: Time, rate: FrameRate): number {
  const num = BigInt(time.num) * BigInt(rate.num);
  const den = BigInt(time.den) * BigInt(rate.den);
  const floor = num >= 0n ? num / den : -((-num + den - 1n) / den);
  return Number(floor);
}

/** The start of frame `frame` at `rate`, exactly. */
export function frameStart(frame: number, rate: FrameRate): Time {
  if (!Number.isSafeInteger(frame)) throw new RangeError('frame must be a safe integer');
  return rational(BigInt(frame) * BigInt(rate.den), BigInt(rate.num));
}

/** `time` moved back to the start of its frame: what the UI sends for a position. */
export function snapToFrame(time: Time, rate: FrameRate): Time {
  return frameStart(frameAt(time, rate), rate);
}

/** Whole frames per second used for timecode labels (30 for 30000/1001). */
export function nominalFps(rate: FrameRate): number {
  return Math.ceil(rate.num / rate.den);
}

const pad = (value: number, width = 2): string => String(value).padStart(width, '0');

/**
 * Non-drop-frame timecode `HH:MM:SS:FF`, counting whole frames at the
 * nominal rate. Negative times get a leading minus sign.
 */
export function formatTimecode(time: Time, rate: FrameRate): string {
  const frame = frameAt(time, rate);
  const fps = nominalFps(rate);
  const sign = frame < 0 ? '-' : '';
  const total = Math.abs(frame);
  const frames = total % fps;
  const seconds = Math.floor(total / fps);
  return `${sign}${pad(Math.floor(seconds / 3600))}:${pad(Math.floor(seconds / 60) % 60)}:${pad(seconds % 60)}:${pad(frames)}`;
}

/** A duration for people: `0:05`, `1:05`, `1:02:05` (whole seconds, rounded down). */
export function formatDuration(time: Time): string {
  const seconds = Number(BigInt(Math.abs(time.num)) / BigInt(time.den));
  const sign = time.num < 0 ? '-' : '';
  const h = Math.floor(seconds / 3600);
  const m = Math.floor(seconds / 60) % 60;
  const s = seconds % 60;
  return h > 0 ? `${sign}${h}:${pad(m)}:${pad(s)}` : `${sign}${m}:${pad(s)}`;
}

/** `true` when `time` is a whole number of flicks (the document's rule). */
export function isOnGrid(time: Time): boolean {
  return FLICKS_PER_SECOND % time.den === 0;
}
