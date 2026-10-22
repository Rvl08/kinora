// The reactive i18n service: the current language and `t()`.

import { en } from './messages/en';
import { id } from './messages/id';
import { detectLocale, translate, translatePlural } from './translate';
import type { Locale, MessageKey, MessageParams, MessageRef, Messages, PluralKey } from './types';

export type { Locale, MessageKey, MessageParams, MessageRef, PluralKey } from './types';
export { detectLocale } from './translate';

const DICTIONARIES: Readonly<Record<Locale, Messages>> = { id, en };

/** Supported languages, in the order the settings list them. */
export const LOCALES: readonly Locale[] = ['id', 'en'];

/**
 * The current language. Reading `t()` inside a component or `$derived`
 * re-renders it when the language changes.
 */
export class I18n {
  #locale = $state<Locale>('en');

  constructor(locale: Locale) {
    this.#locale = locale;
  }

  get locale(): Locale {
    return this.#locale;
  }

  set locale(locale: Locale) {
    this.#locale = locale;
    if (typeof document !== 'undefined') document.documentElement.lang = locale;
  }

  /** The message for `key`. */
  t = (key: MessageKey, params?: MessageParams): string =>
    translate(DICTIONARIES[this.#locale], this.#locale, key, params);

  /** A message prepared elsewhere (e.g. an error description). */
  message = (ref: MessageRef): string => this.t(ref.key, ref.params);

  /** The plural form of `key` for `count`. */
  plural = (key: PluralKey, count: number, params?: MessageParams): string =>
    translatePlural(DICTIONARIES[this.#locale], this.#locale, key, count, params);
}

const STORAGE_KEY = 'kinora.locale';

/** The language chosen in settings, if any (cosmetic storage, docs/07 §10). */
function storedLocale(): Locale | null {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    return value === 'id' || value === 'en' ? value : null;
  } catch {
    return null;
  }
}

/** Remembers the chosen language for the next start. */
export function storeLocale(locale: Locale): void {
  try {
    localStorage.setItem(STORAGE_KEY, locale);
  } catch {
    // Unavailable storage only means the system language applies next time.
  }
}

/** The app-wide instance: the saved language, else the system's. */
export const i18n = new I18n(
  (typeof localStorage === 'undefined' ? null : storedLocale()) ??
    detectLocale(typeof navigator === 'undefined' ? [] : navigator.languages),
);
