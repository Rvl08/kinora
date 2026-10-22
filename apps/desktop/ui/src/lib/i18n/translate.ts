// Pure lookup and formatting; the reactive wrapper is in index.svelte.ts.

import type { Locale, MessageKey, MessageParams, Messages, PluralForms, PluralKey } from './types';

/** Follows a dotted key through a dictionary. */
function lookup(messages: Messages, key: string): unknown {
  let node: unknown = messages;
  for (const part of key.split('.')) {
    if (typeof node !== 'object' || node === null) return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return node;
}

/** Replaces `{name}` placeholders with plain values; numbers are formatted for the locale. */
export function interpolate(
  template: string,
  locale: Locale,
  params?: Readonly<Record<string, string | number>>,
): string {
  if (!params) return template;
  const numbers = new Intl.NumberFormat(locale);
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = params[name];
    if (value === undefined) return match;
    return typeof value === 'number' ? numbers.format(value) : value;
  });
}

/** Translates the `MessageRef` values among `params` (one level deep). */
function resolve(
  messages: Messages,
  locale: Locale,
  params?: MessageParams,
): Readonly<Record<string, string | number>> | undefined {
  if (!params) return undefined;
  const resolved: Record<string, string | number> = {};
  for (const [name, value] of Object.entries(params)) {
    resolved[name] =
      typeof value === 'object' ? translate(messages, locale, value.key, value.params) : value;
  }
  return resolved;
}

/** The message for `key`; the key itself if missing (types make this unreachable). */
export function translate(
  messages: Messages,
  locale: Locale,
  key: MessageKey,
  params?: MessageParams,
): string {
  const template = lookup(messages, key);
  return typeof template === 'string'
    ? interpolate(template, locale, resolve(messages, locale, params))
    : key;
}

/** The plural form of `key` for `count`, with `{count}` filled in. */
export function translatePlural(
  messages: Messages,
  locale: Locale,
  key: PluralKey,
  count: number,
  params?: MessageParams,
): string {
  const forms = lookup(messages, key) as PluralForms | undefined;
  if (!forms) return key;
  const category = new Intl.PluralRules(locale).select(count);
  const template = category === 'one' ? forms.one : forms.other;
  return interpolate(template, locale, { ...resolve(messages, locale, params), count });
}

/** Picks the UI language from the system's preferred languages. */
export function detectLocale(preferred: readonly string[]): Locale {
  for (const tag of preferred) {
    const language = tag.toLowerCase().split('-')[0];
    // `in` is the legacy ISO 639 code for Indonesian, still sent by some systems.
    if (language === 'id' || language === 'in') return 'id';
    if (language === 'en') return 'en';
  }
  return 'en';
}
