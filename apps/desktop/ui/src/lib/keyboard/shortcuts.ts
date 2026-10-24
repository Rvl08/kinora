// The shortcut registry (docs/07 §9). One table of bindings; screens say
// which actions they offer and `dispatch` runs the matching one. Making the
// bindings editable by the user comes later and only changes `SHORTCUTS`.

/** A key combination. `key` is `KeyboardEvent.key`, compared case-insensitively. */
export interface Shortcut {
  readonly key: string;
  readonly ctrl?: boolean;
  readonly shift?: boolean;
  readonly alt?: boolean;
}

export const SHORTCUTS = {
  /** Start the default (vertical) project. */
  newProject: { key: 'n', ctrl: true },
  /** Open a project folder. */
  openProject: { key: 'o', ctrl: true },
  /** Save the open project. */
  saveProject: { key: 's', ctrl: true },
} as const satisfies Readonly<Record<string, Shortcut>>;

export type ShortcutId = keyof typeof SHORTCUTS;

/** The parts of a keyboard event the registry reads. */
export interface KeyInput {
  readonly key: string;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly metaKey: boolean;
  readonly repeat: boolean;
  readonly isComposing?: boolean;
  preventDefault(): void;
}

/** `true` when `input` is exactly `shortcut` (no extra modifiers). */
export function matches(input: KeyInput, shortcut: Shortcut): boolean {
  return (
    input.key.toLowerCase() === shortcut.key.toLowerCase() &&
    input.ctrlKey === (shortcut.ctrl ?? false) &&
    input.shiftKey === (shortcut.shift ?? false) &&
    input.altKey === (shortcut.alt ?? false) &&
    !input.metaKey
  );
}

/**
 * Runs the action bound to `input`, if the screen offers it, and stops the
 * webview's own handling of the key (Ctrl+N would open a browser window,
 * Ctrl+O a file picker, Ctrl+S a "save page" dialog). Held keys do not
 * repeat an action. Returns whether an action ran.
 */
export function dispatch(
  input: KeyInput,
  actions: Partial<Readonly<Record<ShortcutId, () => void>>>,
): boolean {
  if (input.isComposing) return false;
  for (const [id, shortcut] of Object.entries(SHORTCUTS) as [ShortcutId, Shortcut][]) {
    const action = actions[id];
    if (!action || !matches(input, shortcut)) continue;
    input.preventDefault();
    if (!input.repeat) action();
    return true;
  }
  return false;
}

/** The label shown next to an action, e.g. `Ctrl+N`; `ctrl` is the localised key name. */
export function shortcutLabel(id: ShortcutId, ctrl: string): string {
  const shortcut: Shortcut = SHORTCUTS[id];
  const parts = [
    ...(shortcut.ctrl ? [ctrl] : []),
    ...(shortcut.alt ? ['Alt'] : []),
    ...(shortcut.shift ? ['Shift'] : []),
    shortcut.key.toUpperCase(),
  ];
  return parts.join('+');
}

/** The `aria-keyshortcuts` value, e.g. `Control+N`. */
export function ariaShortcut(id: ShortcutId): string {
  const shortcut: Shortcut = SHORTCUTS[id];
  const parts = [
    ...(shortcut.ctrl ? ['Control'] : []),
    ...(shortcut.alt ? ['Alt'] : []),
    ...(shortcut.shift ? ['Shift'] : []),
    shortcut.key.toUpperCase(),
  ];
  return parts.join('+');
}
