// What a drop on the start screen means. Only names are read from a path,
// for display and for a suggested project name; the path itself is passed
// to the shell untouched (docs/07 §10).

import { VIDEO_EXTENSIONS } from './platform';

/** A dropped file or folder. `path` is `null` for browser drops, which carry no paths. */
export interface DroppedItem {
  readonly name: string;
  readonly path: string | null;
}

export type DropIntent =
  /** A `.kinora` project folder: open it. */
  | { readonly kind: 'project'; readonly path: string }
  /** A video: start a vertical project for it. */
  | { readonly kind: 'video'; readonly name: string }
  /** Nothing usable (e.g. a photo or a document). */
  | { readonly kind: 'unsupported'; readonly name: string };

/** The last component of a native path, without trailing separators. */
export function baseName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, '');
  return trimmed.slice(Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\')) + 1);
}

/** A dropped item from a native path. */
export function droppedPath(path: string): DroppedItem {
  return { name: baseName(path), path };
}

const extension = (name: string): string => {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
};

const VIDEO = new Set(VIDEO_EXTENSIONS);

/**
 * The first project folder among the items wins (it can be opened as is);
 * otherwise the first video. Several videos start one project: importing
 * them is Phase 1 work, so only the first name is used.
 */
export function classifyDrop(items: readonly DroppedItem[]): DropIntent | null {
  const project = items.find((item) => item.path !== null && extension(item.name) === 'kinora');
  if (project?.path) return { kind: 'project', path: project.path };
  const video = items.find((item) => VIDEO.has(extension(item.name)));
  if (video) return { kind: 'video', name: video.name };
  const first = items[0];
  return first ? { kind: 'unsupported', name: first.name } : null;
}
