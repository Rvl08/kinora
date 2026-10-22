import { describe, expect, it } from 'vitest';

import { I18n } from './index.svelte';
import { en } from './messages/en';
import { id } from './messages/id';
import { detectLocale, interpolate } from './translate';

/** Every dotted key of a dictionary. */
function keys(node: object, prefix = ''): string[] {
  return Object.entries(node).flatMap(([key, value]) =>
    typeof value === 'string' || 'one' in (value as object)
      ? [`${prefix}${key}`]
      : keys(value as object, `${prefix}${key}.`),
  );
}

describe('dictionaries', () => {
  it('have the same keys in every language', () => {
    expect(keys(en)).toEqual(keys(id));
  });

  it('use the same placeholders in every language', () => {
    const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    const flat = (node: object): string[] =>
      Object.values(node).flatMap((value) =>
        typeof value === 'string' ? [value] : flat(value as object),
      );
    const source = flat(id);
    flat(en).forEach((text, i) => {
      expect(placeholders(text)).toEqual(placeholders(source[i] ?? ''));
    });
  });
});

describe('I18n', () => {
  it('translates, interpolates and switches language', () => {
    const i18n = new I18n('id');
    expect(i18n.t('start.newProject')).toBe('Proyek baru');
    expect(i18n.t('history.undo', { action: 'Potong klip' })).toBe('Urungkan: Potong klip');
    i18n.locale = 'en';
    expect(i18n.t('start.newProject')).toBe('New project');
    expect(i18n.message({ key: 'errors.projectTooNew', params: { found: 3 } })).toContain(
      'format 3',
    );
  });

  it('chooses plural forms per language', () => {
    const i18n = new I18n('en');
    expect(i18n.plural('units.clips', 1)).toBe('1 clip');
    expect(i18n.plural('units.clips', 1200)).toBe('1,200 clips');
    i18n.locale = 'id';
    expect(i18n.plural('units.clips', 1200)).toBe('1.200 klip');
  });

  it('leaves unknown placeholders visible rather than dropping text', () => {
    expect(interpolate('Halo {name}', 'id', {})).toBe('Halo {name}');
  });
});

describe('detectLocale', () => {
  it('prefers Indonesian when the system asks for it', () => {
    expect(detectLocale(['id-ID', 'en-US'])).toBe('id');
    expect(detectLocale(['in'])).toBe('id');
    expect(detectLocale(['en-GB', 'id'])).toBe('en');
    expect(detectLocale(['fr-FR'])).toBe('en');
    expect(detectLocale([])).toBe('en');
  });
});
