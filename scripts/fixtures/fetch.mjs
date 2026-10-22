// Downloads the freely licensed fixtures listed in manifest.json
// (kind "download") into tests/media/fixtures, which git ignores.
//
// Usage: node scripts/fixtures/fetch.mjs [--ids a,b] [--force] [--keep-sources] [--pin]
//
// Only the pinned URLs are used, and every download is checked against its
// pinned size and SHA-256; a mismatch is refused and the bytes are deleted.
// Some sources are byte ranges of a larger file (the first minutes of a long
// recording); the range is what is pinned. Interrupted downloads resume from
// the `.part` file. Downloads are data: they land in their own directory
// under .sources/, are never executed, and are only read by FFmpeg or tar.
//
// A fixture with a `transform` (cut an audio excerpt, extract archive
// members) is derived from its source with the local FFmpeg or tar. Those
// outputs depend on the FFmpeg build; verify.mjs explains how they are
// checked. --pin writes their hashes into the manifest (maintainers only).
//
// Idempotent: fixtures whose files already match are kept.

import { createWriteStream, existsSync } from 'node:fs';
import { mkdir, rename, rm, stat } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
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
  run,
  selectFixtures,
  sha256File,
  sourcesDir,
} from './lib.mjs';

const USER_AGENT = 'KinoraFixtures/1.0 (+https://github.com/Rvl08/kinora; test media fetcher)';
const ATTEMPTS = 5;

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

/** True when `path` holds exactly the pinned bytes of `source`. */
async function matches(path, source) {
  if (!existsSync(path)) return false;
  if ((await stat(path)).size !== source.bytes) return false;
  return (await sha256File(path)) === source.sha256;
}

/**
 * Downloads `source` (optionally a byte range) to `target` via `target.part`,
 * resuming a previous partial download, then checks size and SHA-256.
 */
async function download(source, target) {
  if (!source.sha256 || !source.bytes) throw new Error(`no pinned sha256/bytes for ${source.url}`);
  await mkdir(dirname(target), { recursive: true });
  const part = `${target}.part`;
  const [first, last] = source.range ?? [0, source.bytes - 1];
  if (last - first + 1 !== source.bytes)
    throw new Error(`range and bytes disagree for ${source.url}`);

  for (let attempt = 1; attempt <= ATTEMPTS; attempt++) {
    const have = existsSync(part) ? (await stat(part)).size : 0;
    if (have > source.bytes) await rm(part);
    const offset = have > source.bytes ? 0 : have;
    if (offset < source.bytes) {
      const ranged = source.range || offset > 0;
      const headers = { 'User-Agent': USER_AGENT };
      if (ranged) headers.Range = `bytes=${first + offset}-${last}`;
      console.log(`  GET ${source.url}${ranged ? ` [${first + offset}-${last}]` : ''}`);
      try {
        const response = await fetch(source.url, { headers, redirect: 'follow' });
        if (response.status === 429 || response.status >= 500) {
          const wait = Number(response.headers.get('retry-after')) || 10 * attempt;
          console.warn(`  HTTP ${response.status}, retrying in ${wait} s`);
          await response.body?.cancel();
          await sleep(wait * 1000);
          continue;
        }
        if (ranged && response.status !== 206) {
          await response.body?.cancel();
          if (source.range)
            throw new Error(`server ignored the byte range (HTTP ${response.status})`);
          await rm(part, { force: true }); // no resume support: start over
          continue;
        }
        if (!ranged && response.status !== 200) throw new Error(`HTTP ${response.status}`);
        await pipeline(
          Readable.fromWeb(response.body),
          createWriteStream(part, { flags: offset > 0 ? 'a' : 'w' }),
        );
      } catch (error) {
        console.warn(`  attempt ${attempt} failed: ${error.message}`);
        await sleep(3000 * attempt);
        continue;
      }
    }
    if (await matches(part, source)) {
      await rename(part, target);
      return;
    }
    await rm(part, { force: true });
    throw new Error(`refused ${source.url}: size or SHA-256 differs from manifest.json`);
  }
  throw new Error(`giving up on ${source.url} after ${ATTEMPTS} attempts`);
}

/** Substitutes {src} and {out} in transform arguments. */
const fill = (args, src, out) =>
  args.map((a) => a.replaceAll('{src}', src).replaceAll('{out}', out));

async function transform(fixture, src, work) {
  const { ffmpeg: args, untar } = fixture.transform;
  if (args) {
    const file = fixture.files[0];
    const final = fixtureFile(file);
    const out = join(work, `out-${basename(final)}`);
    await mkdir(dirname(final), { recursive: true });
    run(ffmpeg, ['-hide_banner', '-loglevel', 'error', '-y', ...fill(args, src, out)]);
    await rename(out, final);
  } else if (untar) {
    const staging = join(work, 'extract');
    await rm(staging, { recursive: true, force: true });
    await mkdir(staging, { recursive: true });
    // Windows' bsdtar by full path (a GNU tar from Git Bash reads "D:\" as a
    // host). bsdtar refuses absolute and ".." member paths by default.
    const tar =
      process.platform === 'win32'
        ? join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe')
        : 'tar';
    run(tar, ['-xzf', src, '-C', staging, ...fixture.files.map((f) => f.member)]);
    for (const file of fixture.files) {
      const final = fixtureFile(file);
      await mkdir(dirname(final), { recursive: true });
      await rename(join(staging, ...file.member.split('/')), final);
    }
    await rm(staging, { recursive: true, force: true });
  } else {
    throw new Error(`${fixture.id}: unknown transform`);
  }
}

requireFfmpeg();
const manifest = await loadManifest();
const fixtures = selectFixtures(manifest, 'download');
const state = await loadState();
const build = ffmpegBuild();
const force = flag('force');
const failures = [];
const started = Date.now();

for (const fixture of fixtures) {
  if (!force && (await isCurrent(fixture, state, build))) {
    console.log(`keep  ${fixture.id}`);
    continue;
  }
  console.log(`fetch ${fixture.id}`);
  const t0 = Date.now();
  try {
    if (!fixture.transform) {
      // The download is the fixture.
      const target = fixtureFile(fixture.files[0]);
      if (force || !(await matches(target, fixture.source))) await download(fixture.source, target);
    } else {
      const work = join(sourcesDir, fixture.id);
      const src = join(work, fixture.source.name);
      if (!(await matches(src, fixture.source))) await download(fixture.source, src);
      await transform(fixture, src, work);
      // The source is only needed to derive the fixture again.
      if (!flag('keep-sources')) await rm(work, { recursive: true, force: true });
    }
    await recordState(state, fixture, build, null);
    // Archive members are exact bytes and pinned by hand; FFmpeg outputs are pinned here.
    if (flag('pin') && fixture.transform?.ffmpeg) await pin(manifest, fixture, build, null);
    console.log(`  done in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  } catch (error) {
    console.error(`  FAILED: ${error.message}`);
    failures.push(fixture.id);
  }
}

console.log(`\nfetch finished in ${((Date.now() - started) / 1000).toFixed(0)} s`);
if (failures.length) {
  console.error(`Failed: ${failures.join(', ')}`);
  process.exit(1);
}
console.log('Run `node scripts/fixtures/verify.mjs` to check every fixture.');
