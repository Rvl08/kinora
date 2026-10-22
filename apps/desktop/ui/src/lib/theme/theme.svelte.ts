// Light, dark or follow the system (docs/07 §7.1). This module only decides
// which theme applies and marks it on <html data-theme>; the colours are the
// semantic tokens in tokens.css, designed with Impeccable.

export type ThemePreference = 'system' | 'light' | 'dark';
export type Theme = 'light' | 'dark';

/** The theme a preference resolves to. */
export function resolveTheme(preference: ThemePreference, systemPrefersDark: boolean): Theme {
  if (preference === 'system') return systemPrefersDark ? 'dark' : 'light';
  return preference;
}

const DARK_QUERY = '(prefers-color-scheme: dark)';

/** Tracks the preference and the system setting, and applies the result. */
export class ThemeController {
  #preference = $state<ThemePreference>('system');
  #systemDark = $state(false);

  constructor(preference: ThemePreference = 'system') {
    this.#preference = preference;
  }

  get preference(): ThemePreference {
    return this.#preference;
  }

  set preference(preference: ThemePreference) {
    this.#preference = preference;
    this.#apply();
  }

  get theme(): Theme {
    return resolveTheme(this.#preference, this.#systemDark);
  }

  /** Starts following the system setting; returns a function that stops it. */
  start(): () => void {
    if (typeof window === 'undefined' || !window.matchMedia) return () => {};
    const query = window.matchMedia(DARK_QUERY);
    const update = () => {
      this.#systemDark = query.matches;
      this.#apply();
    };
    update();
    query.addEventListener('change', update);
    return () => query.removeEventListener('change', update);
  }

  #apply(): void {
    if (typeof document === 'undefined') return;
    document.documentElement.dataset['theme'] = this.theme;
    document.documentElement.style.colorScheme = this.theme;
  }
}

const STORAGE_KEY = 'kinora.theme';

/** The preference saved in this window's storage (cosmetic only, docs/07 §10). */
export function storedPreference(): ThemePreference {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    return value === 'light' || value === 'dark' ? value : 'system';
  } catch {
    return 'system';
  }
}

/** Saves the preference; losing it only means the system setting applies. */
export function storePreference(preference: ThemePreference): void {
  try {
    localStorage.setItem(STORAGE_KEY, preference);
  } catch {
    // Storage can be unavailable (private mode, policy); nothing to do.
  }
}

/** The app-wide theme. */
export const theme = new ThemeController(
  typeof localStorage === 'undefined' ? 'system' : storedPreference(),
);
