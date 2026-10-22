// Generates the synthetic fixtures listed in manifest.json (kind
// "generate") with the repository's LGPL FFmpeg (vendor/ffmpeg-lgpl) into
// tests/media/fixtures, which git ignores. They are our own work, so they
// carry no third-party licence.
//
// Usage: node scripts/fixtures/generate.mjs [--ids a,b] [--force] [--allow-skip] [--pin]
//
// Each recipe lists encoders in order of preference. The first one that
// encodes a test frame on this machine is used; the NVENC ones need an NVIDIA
// GPU, so a runner without one falls back to a software encoder or, when a
// fixture has none (HEVC Main 10 HDR), skips it: exit code 1, or 0 with
// --allow-skip. Software encoders give byte-identical files for the same
// FFmpeg build; hardware encoders do not promise that across GPUs and
// drivers, which is why verify.mjs also checks properties with ffprobe.
//
// Recipe placeholders: {out} output file, {tmp} scratch directory,
// {encoder} the chosen encoder's arguments, {font} a monospace font and
// {file:path} a repository file (both escaped for filter graphs),
// {fixture:id} another fixture's file. A "heif" step wraps HEVC tiles into
// a HEIC file (heif.mjs).
//
// Idempotent: fixtures whose files already match are kept (--force redoes).

import { existsSync } from 'node:fs';
import { mkdir, rename, rm } from 'node:fs/promises';
import { dirname, extname, join } from 'node:path';
import { writeHeicGrid } from './heif.mjs';
import {
  ffmpeg,
  ffmpegBuild,
  fixtureFile,
  flag,
  isCurrent,
  loadManifest,
  loadState,
  pin,
  recordState,
  requireFfmpeg,
  root,
  run,
  selectFixtures,
  tmpDir,
} from './lib.mjs';

/** Escapes a path for use inside a quoted filter-graph option value. */
const filterPath = (path) => path.replaceAll('\\', '/').replaceAll(':', '\\:');

function fontPath() {
  const candidates =
    process.platform === 'win32'
      ? [join(process.env.SystemRoot ?? 'C:\\Windows', 'Fonts', 'consola.ttf')]
      : ['/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf', '/System/Library/Fonts/Menlo.ttc'];
  const font = process.env.KINORA_FIXTURE_FONT ?? candidates.find((p) => existsSync(p));
  if (!font) throw new Error('no monospace font found; set KINORA_FIXTURE_FONT');
  return font;
}

const encoderProbe = new Map();

/** Whether `name` with `args` can encode a few frames on this machine. */
function encoderWorks(name, args) {
  const key = JSON.stringify(args);
  if (!encoderProbe.has(key)) {
    try {
      const input = ['-f', 'lavfi', '-i', 'testsrc2=size=640x480:rate=30', '-frames:v', '3'];
      run(ffmpeg, ['-hide_banner', '-loglevel', 'error', ...input, ...args, '-f', 'null', '-']);
      encoderProbe.set(key, true);
    } catch {
      console.log(`  encoder ${name} is not usable here`);
      encoderProbe.set(key, false);
    }
  }
  return encoderProbe.get(key);
}

function chooseEncoder(recipe) {
  for (const [name, args] of Object.entries(recipe.encoders ?? {})) {
    if (encoderWorks(name, args)) return { name, args };
  }
  return null;
}

function expand(value, context) {
  return value
    .replaceAll('{out}', context.out)
    .replaceAll('{tmp}', context.tmp)
    .replaceAll('{font}', () => filterPath(fontPath()))
    .replace(/\{file:([^}]+)\}/g, (_, rel) => filterPath(join(root, ...rel.split('/'))))
    .replace(/\{fixture:([^}]+)\}/g, (_, id) => {
      const other = context.manifest.fixtures.find((f) => f.id === id);
      if (!other) throw new Error(`unknown fixture ${id}`);
      const path = fixtureFile(other.files[0]);
      if (!existsSync(path)) throw new Error(`needs fixture ${id}: run fetch.mjs first`);
      return path;
    });
}

async function runSteps(fixture, encoder, context) {
  for (const step of fixture.recipe.steps) {
    if (step.ffmpeg) {
      const args = step.ffmpeg.flatMap((a) =>
        a === '{encoder}' ? encoder.args : [expand(a, context)],
      );
      run(ffmpeg, ['-hide_banner', '-loglevel', 'error', '-y', ...args]);
    } else if (step.heif) {
      const { input, ...grid } = step.heif;
      await writeHeicGrid({ hevcPath: expand(input, context), outPath: context.out, ...grid });
    } else {
      throw new Error(`${fixture.id}: unknown step ${Object.keys(step)}`);
    }
  }
}

requireFfmpeg();
const manifest = await loadManifest();
const fixtures = selectFixtures(manifest, 'generate');
const state = await loadState();
const build = ffmpegBuild();
const force = flag('force');
const failed = [];
const skipped = [];
const started = Date.now();
console.log(`FFmpeg ${build}`);

for (const fixture of fixtures) {
  if (!force && (await isCurrent(fixture, state, build))) {
    console.log(`keep  ${fixture.id}`);
    continue;
  }
  console.log(`make  ${fixture.id}`);
  const encoder = fixture.recipe.encoders
    ? chooseEncoder(fixture.recipe)
    : { name: null, args: [] };
  if (!encoder) {
    console.warn(`  SKIP: none of ${Object.keys(fixture.recipe.encoders).join(', ')} works here`);
    skipped.push(fixture.id);
    continue;
  }
  if (encoder.name) console.log(`  encoder ${encoder.name}`);

  const final = fixtureFile(fixture.files[0]);
  const tmp = join(tmpDir, fixture.id);
  const out = join(tmp, `out${extname(final)}`);
  const t0 = Date.now();
  try {
    await rm(tmp, { recursive: true, force: true });
    await mkdir(tmp, { recursive: true });
    await mkdir(dirname(final), { recursive: true });
    await runSteps(fixture, encoder, { out, tmp, manifest });
    await rename(out, final);
    await rm(tmp, { recursive: true, force: true });
    await recordState(state, fixture, build, encoder.name);
    if (flag('pin')) await pin(manifest, fixture, build, encoder.name);
    console.log(`  done in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  } catch (error) {
    console.error(`  FAILED: ${error.message}`);
    failed.push(fixture.id);
  }
}

await rm(tmpDir, { recursive: true, force: true });
console.log(`\ngenerate finished in ${((Date.now() - started) / 1000).toFixed(0)} s`);
if (skipped.length) console.warn(`Skipped (no usable encoder): ${skipped.join(', ')}`);
if (failed.length) console.error(`Failed: ${failed.join(', ')}`);
if (failed.length || (skipped.length && !flag('allow-skip'))) process.exit(1);
console.log('Run `node scripts/fixtures/verify.mjs` to check every fixture.');
