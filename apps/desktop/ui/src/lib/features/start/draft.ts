// A new project being named on the start screen.

import type { Locale } from '$lib/i18n/index.svelte';

import { defaultProjectName, nameFromFile } from './project-name';
import type { RatioPreset } from './ratio';

export interface Draft {
  readonly preset: RatioPreset;
  /** The suggested name. */
  readonly name: string;
  /** A dropped video that will not be added (no media import yet). */
  readonly skippedVideo: string | null;
}

/**
 * A draft for `preset`. A dropped video suggests its own name and is kept
 * only to tell the person it was not added.
 */
export function newDraft(
  preset: RatioPreset,
  context: { prefix: string; now: number; locale: Locale },
  video: string | null = null,
): Draft {
  const suggested = video === null ? null : nameFromFile(video);
  return {
    preset,
    name: suggested ?? defaultProjectName(context.prefix, context.now, context.locale),
    skippedVideo: video,
  };
}
