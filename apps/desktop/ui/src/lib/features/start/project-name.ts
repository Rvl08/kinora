// Project names. The name is also the folder name (`<name>.kinora`), so it
// must be a valid Windows file name. The shell is the authority and checks
// again (crates/kinora-shell, name.rs); these are the same rules, so the
// person learns what is wrong while typing, in their language.

import type { IpcError } from '$lib/ipc/bindings';
import type { Locale, MessageRef } from '$lib/i18n/index.svelte';

/** Longest name accepted, in characters (the shell's `MAX_NAME_CHARS`). */
export const MAX_NAME_LENGTH = 100;

/** Characters Windows does not allow in file names, as shown to people. */
export const FORBIDDEN_SHOWN = '\\ / : * ? " < > |';

// Forbidden: the Windows set, control characters, and text-direction
// overrides that can make a name display as something else.
// eslint-disable-next-line no-control-regex -- control characters are exactly what is refused
const FORBIDDEN = /[\\/:*?"<>|\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/;
// eslint-disable-next-line no-control-regex -- see above
const FORBIDDEN_ALL = /[\\/:*?"<>|\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]+/g;

// Device names, whatever follows the first dot and with spaces before it
// (`nul.kinora`, `COM1 .x`).
const DEVICE = /^(con|prn|aux|nul|conin\$|conout\$|com[0-9¹²³]|lpt[0-9¹²³])$/i;

const length = (text: string): number => [...text].length;

function isReserved(name: string): boolean {
  const stem = (name.split('.')[0] ?? name).replace(/ +$/, '');
  return DEVICE.test(stem);
}

/** The name that will be sent: surrounding spaces are not part of it. */
export function normaliseName(raw: string): string {
  return raw.trim();
}

/** Why `raw` cannot be a project name, or `null` when it can. */
export function checkProjectName(raw: string): MessageRef | null {
  const name = normaliseName(raw);
  if (name === '') return { key: 'start.form.nameEmpty' };
  if (length(name) > MAX_NAME_LENGTH) {
    return { key: 'start.form.nameTooLong', params: { max: MAX_NAME_LENGTH } };
  }
  if (FORBIDDEN.test(name)) {
    return { key: 'start.form.nameForbidden', params: { chars: FORBIDDEN_SHOWN } };
  }
  if (name.endsWith('.')) return { key: 'start.form.nameTrailingDot' };
  if (isReserved(name)) return { key: 'start.form.nameReserved' };
  return null;
}

/** Cuts `name` to the length limit without splitting a character. */
function clip(name: string): string {
  return [...name].slice(0, MAX_NAME_LENGTH).join('').trim().replace(/\.+$/, '');
}

/** The suggested name of a new project: "Proyek 24 Okt 2026" / "Project Oct 24, 2026". */
export function defaultProjectName(prefix: string, now: number, locale: Locale): string {
  const date = new Intl.DateTimeFormat(locale, {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  }).format(now);
  return clip(`${prefix} ${date}`.replace(FORBIDDEN_ALL, ' ').replace(/\s+/g, ' '));
}

/**
 * A project name suggested by a video's file name: the extension dropped and
 * characters a folder cannot carry replaced by spaces. `null` when nothing
 * usable is left.
 */
export function nameFromFile(fileName: string): string | null {
  const dot = fileName.lastIndexOf('.');
  const stem = dot > 0 ? fileName.slice(0, dot) : fileName;
  const name = clip(stem.replace(FORBIDDEN_ALL, ' ').replace(/\s+/g, ' '));
  return name === '' || checkProjectName(name) !== null ? null : name;
}

/** Where a `create_project` failure is shown: on the name field, or for the whole form. */
export function createErrorTarget(error: IpcError): 'name' | 'form' {
  if (error.code === 'io') {
    return error.kind === 'invalidName' || error.kind === 'alreadyExists' ? 'name' : 'form';
  }
  if (error.code === 'invalidArgument') return error.argument === 'name' ? 'name' : 'form';
  return 'form';
}

/** The message for a name the shell refused; other errors use `describeError`. */
export function describeNameError(error: IpcError): MessageRef | null {
  if (error.code === 'io' && error.kind === 'alreadyExists') return { key: 'start.form.nameTaken' };
  if (createErrorTarget(error) === 'name') return { key: 'start.form.nameInvalid' };
  return null;
}
