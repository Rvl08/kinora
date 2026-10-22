// Checks every fixture in tests/media/fixtures against manifest.json and
// prints a table. Exit code 1 when a fixture is missing, corrupt or has the
// wrong properties.
//
// Usage: node scripts/fixtures/verify.mjs [--ids a,b] [--allow-missing]
//
// Two checks per file:
//   - SHA-256. Downloads (and archive members) are exact bytes: any
//     difference fails. Files made by FFmpeg (generated fixtures and audio
//     excerpts) are pinned for the FFmpeg build and encoder in their `pin`
//     entry. A difference fails only when this machine made the file with
//     that same build and the same software encoder; with another build or a
//     hardware encoder (NVENC output varies with GPU and driver) it is
//     reported as "differs" and the property check decides.
//   - Properties with ffprobe (the `expect` block): container, codec,
//     profile, size, pixel format, frame rate and VFR, colour tags, HDR10
//     metadata, alpha, EXIF rotation, ICC profile, tile grid, duration.
//
// --allow-missing reports absent fixtures without failing (for example on a
// runner without an NVIDIA GPU, where generate.mjs skips the HDR10 clip).

import { existsSync } from 'node:fs';
import { stat } from 'node:fs/promises';
import {
  checkProperties,
  ffmpegBuild,
  fixtureFile,
  flag,
  formatBytes,
  isHardwareEncoder,
  loadManifest,
  loadState,
  printTable,
  requireFfmpeg,
  selectFixtures,
  sha256File,
} from './lib.mjs';

/** Exact bytes from the internet, not produced by the local FFmpeg. */
const isExact = (fixture) => fixture.kind === 'download' && !fixture.transform?.ffmpeg;

async function hashStatus(fixture, file, record) {
  const digest = await sha256File(fixtureFile(file));
  if (!file.sha256) return { status: 'unpinned', fail: false };
  if (digest === file.sha256) return { status: 'ok', fail: false };
  if (isExact(fixture)) return { status: 'MISMATCH', fail: true };
  if (!record) return { status: 'differs', fail: false, note: 'origin unknown: no local record' };
  if (record.files?.[file.path] !== digest) {
    return { status: 'MISMATCH', fail: true, note: 'changed since fetch/generate made it' };
  }
  const encoder = record.encoder ?? null;
  if (encoder && isHardwareEncoder(encoder)) {
    return { status: 'differs', fail: false, note: `hardware encoder ${encoder}` };
  }
  if (record.ffmpeg === fixture.pin?.ffmpeg && encoder === (fixture.pin?.encoder ?? null)) {
    return { status: 'MISMATCH', fail: true, note: 'same FFmpeg build and encoder as the pin' };
  }
  const by = `${record.ffmpeg}${encoder ? ` / ${encoder}` : ''}`;
  return { status: 'differs', fail: false, note: `made with ${by}, pinned for another build` };
}

requireFfmpeg();
const manifest = await loadManifest();
const fixtures = selectFixtures(manifest);
const state = await loadState();
const allowMissing = flag('allow-missing');
const rows = [];
const notes = [];
let failures = 0;
let total = 0;

console.log(`FFmpeg ${ffmpegBuild()}\n`);
for (const fixture of fixtures) {
  const record = state[fixture.id];
  let bytes = 0;
  let missing = 0;
  let fail = false;
  const hashes = new Set();
  let props = fixture.expect ? 'ok' : '-';

  for (const file of fixture.files) {
    const path = fixtureFile(file);
    if (!existsSync(path)) {
      missing++;
      continue;
    }
    bytes += (await stat(path)).size;
    const hash = await hashStatus(fixture, file, record);
    hashes.add(hash.status);
    fail ||= hash.fail;
    if (hash.note) notes.push(`${fixture.id}: ${file.path}: sha256 ${hash.status} (${hash.note})`);
    if (fixture.expect) {
      const problems = checkProperties(path, fixture.expect);
      if (problems.length) {
        props = 'FAIL';
        fail = true;
        for (const p of problems) notes.push(`${fixture.id}: ${file.path}: ${p}`);
      }
    }
  }

  total += bytes;
  const where =
    fixture.files.length === 1 ? fixture.files[0].path : `${fixture.files.length} files`;
  let result = fail ? 'FAIL' : 'ok';
  if (missing) {
    result = allowMissing && !fail ? 'missing' : 'FAIL';
    notes.push(`${fixture.id}: ${missing} of ${fixture.files.length} file(s) missing`);
  }
  if (result === 'FAIL') failures++;
  const hash = missing === fixture.files.length ? '-' : [...hashes].sort().join('/');
  rows.push([
    fixture.id,
    where,
    missing === fixture.files.length ? '-' : formatBytes(bytes),
    hash,
    missing === fixture.files.length ? '-' : props,
    record?.encoder ?? '',
    result,
  ]);
}

printTable(['fixture', 'file', 'size', 'sha256', 'properties', 'encoder', 'result'], rows);
console.log(`\n${rows.length} fixtures, ${formatBytes(total)} on disk`);
if (notes.length) console.log(`\n${notes.join('\n')}`);
if (failures) {
  console.error(`\n${failures} fixture(s) failed verification.`);
  process.exit(1);
}
