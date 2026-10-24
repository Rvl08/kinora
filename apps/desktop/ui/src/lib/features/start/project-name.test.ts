import { describe, expect, it } from 'vitest';

import type { IpcError } from '$lib/ipc/bindings';
import { describeError } from '$lib/ipc/errors';
import { I18n } from '$lib/i18n/index.svelte';

import { newDraft } from './draft';
import {
  MAX_NAME_LENGTH,
  checkProjectName,
  createErrorTarget,
  defaultProjectName,
  describeNameError,
  nameFromFile,
} from './project-name';

describe('checkProjectName', () => {
  it('accepts ordinary names, spaces and non-Latin letters', () => {
    for (const name of [
      'Liburan di Bali',
      'Resep #2 (final)',
      'Vlog — Jakarta',
      'café',
      '旅行',
      'video.v2',
      '.hidden',
      'COM10',
      'Console',
    ]) {
      expect(checkProjectName(name)).toBeNull();
    }
  });

  it('explains what Windows would refuse', () => {
    expect(checkProjectName('   ')).toEqual({ key: 'start.form.nameEmpty' });
    expect(checkProjectName('a/b')?.key).toBe('start.form.nameForbidden');
    expect(checkProjectName('a:b')?.key).toBe('start.form.nameForbidden');
    expect(checkProjectName('tab\there')?.key).toBe('start.form.nameForbidden');
    expect(checkProjectName('CON')).toEqual({ key: 'start.form.nameReserved' });
    expect(checkProjectName('lpt1')).toEqual({ key: 'start.form.nameReserved' });
    expect(checkProjectName('nul.kinora')).toEqual({ key: 'start.form.nameReserved' });
    expect(checkProjectName('COM1 .x')).toEqual({ key: 'start.form.nameReserved' });
    expect(checkProjectName('conin$')).toEqual({ key: 'start.form.nameReserved' });
    expect(checkProjectName('akhir.')).toEqual({ key: 'start.form.nameTrailingDot' });
    expect(checkProjectName('..')).toEqual({ key: 'start.form.nameTrailingDot' });
    expect(checkProjectName(String.raw`a\b`)?.key).toBe('start.form.nameForbidden');
    expect(checkProjectName(`evil${String.fromCharCode(0x202e)}txt`)?.key).toBe(
      'start.form.nameForbidden',
    );
    expect(checkProjectName('x'.repeat(MAX_NAME_LENGTH + 1))?.key).toBe('start.form.nameTooLong');
    expect(checkProjectName('x'.repeat(MAX_NAME_LENGTH))).toBeNull();
  });
});

describe('suggested names', () => {
  const now = new Date(2026, 9, 24, 14, 5).getTime();

  it('dates the default name in the UI language, without characters a folder refuses', () => {
    expect(defaultProjectName('Proyek', now, 'id')).toBe('Proyek 24 Okt 2026');
    const english = defaultProjectName('Project', now, 'en');
    expect(english).toMatch(/^Project Oct 24, 2026$/);
    expect(checkProjectName(english)).toBeNull();
  });

  it('takes a dropped video’s name without its extension', () => {
    expect(nameFromFile('Pantai Kuta.mp4')).toBe('Pantai Kuta');
    expect(nameFromFile('a:b?.mov')).toBe('a b');
    expect(nameFromFile('???.mp4')).toBeNull();
    expect(nameFromFile('CON.mp4')).toBeNull();
  });

  it('builds a draft from a dropped video, or from the date', () => {
    const context = { prefix: 'Proyek', now, locale: 'id' as const };
    expect(newDraft('vertical', context, 'Pantai.mp4')).toEqual({
      preset: 'vertical',
      name: 'Pantai',
      skippedVideo: 'Pantai.mp4',
    });
    expect(newDraft('square', context)).toEqual({
      preset: 'square',
      name: 'Proyek 24 Okt 2026',
      skippedVideo: null,
    });
  });
});

describe('create_project errors', () => {
  const name: IpcError[] = [
    { code: 'io', kind: 'alreadyExists' },
    { code: 'io', kind: 'invalidName' },
    { code: 'invalidArgument', argument: 'name', reason: 'reserved' },
  ];
  const form: IpcError[] = [
    { code: 'io', kind: 'permissionDenied' },
    { code: 'io', kind: 'storageFull' },
    { code: 'invalidArgument', argument: 'parentDir', reason: 'not a folder' },
    { code: 'internal', incident: '0192' },
  ];

  it('places name problems on the field and the rest on the form', () => {
    for (const error of name) expect(createErrorTarget(error)).toBe('name');
    for (const error of form) expect(createErrorTarget(error)).toBe('form');
    expect(describeNameError({ code: 'io', kind: 'alreadyExists' })).toEqual({
      key: 'start.form.nameTaken',
    });
    expect(describeNameError({ code: 'io', kind: 'storageFull' })).toBeNull();
  });

  it('has a message for each in both languages', () => {
    for (const locale of ['id', 'en'] as const) {
      const i18n = new I18n(locale);
      for (const error of name) {
        const ref = describeNameError(error);
        expect(ref).not.toBeNull();
        if (ref) expect(i18n.message(ref)).not.toMatch(/^start\.|\{/);
      }
      for (const error of form) {
        expect(i18n.message(describeError(error))).not.toMatch(/^errors\.|\{/);
      }
      expect(
        i18n.message({ key: 'start.form.nameForbidden', params: { chars: '\\ /' } }),
      ).toContain('\\ /');
    }
  });
});
