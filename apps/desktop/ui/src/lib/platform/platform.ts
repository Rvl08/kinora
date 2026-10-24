// What the UI needs from the desktop beyond IPC commands: native file
// dialogs and files dropped on the window. Every path the UI ever sends to
// the shell comes from here or from the recent-projects list; the UI never
// builds a path itself (docs/07 §10).

import type { Unlisten } from '$lib/ipc/transport';

/** Why a folder is asked for. */
export type FolderPurpose = 'projectLocation' | 'openProject' | 'locateProject';

/** Options of a native folder or file dialog. */
export interface DialogOptions {
  /** The dialog's title, in the UI language. */
  title: string;
  /** Where the dialog starts; a path a previous dialog returned. */
  defaultPath?: string | undefined;
}

/** Options of the folder dialog. */
export interface FolderDialogOptions extends DialogOptions {
  readonly purpose: FolderPurpose;
}

/** Options of the video file dialog. */
export interface VideoDialogOptions extends DialogOptions {
  /** The file-type filter's label, e.g. "Video". */
  filterName: string;
}

/** Follows files dragged over and dropped on the window. */
export interface DropHandlers {
  /** `true` while files are dragged over the window. */
  hover(active: boolean): void;
  /** Files or folders were dropped; native paths. */
  drop(paths: readonly string[]): void;
}

export interface Platform {
  /** Asks for a folder; `null` when the dialog is cancelled. */
  pickFolder(options: FolderDialogOptions): Promise<string | null>;
  /** Asks for one video file; `null` when the dialog is cancelled. */
  pickVideo(options: VideoDialogOptions): Promise<string | null>;
  /**
   * Follows native drag and drop. `null` means the platform has none, and
   * the page's own HTML drag events carry drops (a browser during
   * development; they carry file names but no paths).
   */
  watchDrops(handlers: DropHandlers): Promise<Unlisten> | null;
}

/** Video containers the file dialog offers (FFmpeg reads all of them). */
export const VIDEO_EXTENSIONS: readonly string[] = [
  'mp4',
  'mov',
  'm4v',
  'mkv',
  'webm',
  'avi',
  'mts',
  'm2ts',
  '3gp',
];

/** A platform without dialogs, for a build that runs outside the shell. */
export const UNAVAILABLE: Platform = {
  pickFolder: () => Promise.reject(new Error('no native dialogs outside the Kinora shell')),
  pickVideo: () => Promise.reject(new Error('no native dialogs outside the Kinora shell')),
  watchDrops: () => null,
};
