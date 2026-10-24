// Verifies that an FFmpeg build is LGPL (ADR-0005) and reports what it can do.
//
// Usage:
//   pnpm run ffmpeg:verify [--ship] [--json <path>]
//     Development build in vendor/ffmpeg-lgpl (BtbN, shared DLLs), checked by
//     running its ffmpeg.exe.
//   node scripts/ffmpeg/verify.mjs --release [<dir>] [--json <path>]
//     Kinora's static release build (P0-05b, default vendor/ffmpeg-static),
//     checked without running anything: release/verify-release.mjs reads the
//     static libraries themselves. Always ship-strict.
//
// Development mode always fails on: --enable-gpl, --enable-nonfree, a GPL
// external library, a licence text other than the LGPL, a static build, or a
// version different from lock.json. Ship blockers (libraries whose licence or
// patent position is not cleared for distribution) are warnings for
// development and failures with --ship. Missing optional capabilities are
// reported, not failed.

import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { BANNED, SHIP_BLOCKERS } from './policy.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const jsonIndex = process.argv.indexOf('--json');
const jsonPath = jsonIndex === -1 ? null : resolve(process.argv[jsonIndex + 1]);

const releaseIndex = process.argv.indexOf('--release');
if (releaseIndex !== -1) {
  const next = process.argv[releaseIndex + 1];
  const dir =
    next && !next.startsWith('--') ? resolve(next) : join(root, 'vendor', 'ffmpeg-static');
  const { verifyRelease } = await import('./release/verify-release.mjs');
  process.exit(await verifyRelease({ root, dir, jsonPath }));
}

const lock = JSON.parse(await readFile(join(root, 'scripts', 'ffmpeg', 'lock.json'), 'utf8'));
const prefix = join(root, 'vendor', 'ffmpeg-lgpl');
const exe = join(prefix, 'bin', process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg');
const ship = process.argv.includes('--ship');

// Capabilities ADR-0005 relies on.
const WANT = {
  encoders: [
    'h264_nvenc',
    'hevc_nvenc',
    'av1_nvenc',
    'h264_qsv',
    'hevc_qsv',
    'av1_qsv',
    'h264_amf',
    'hevc_amf',
    'av1_amf',
    'libopenh264',
    'libsvtav1',
    'libvpx-vp9',
    'prores_ks',
    'dnxhd',
    'aac',
    'libopus',
    'flac',
    'libmp3lame',
  ],
  decoders: ['libdav1d', 'h264', 'hevc', 'prores', 'vp9', 'av1'],
  hwaccels: ['d3d11va', 'd3d12va', 'dxva2', 'cuda', 'qsv'],
  filters: ['deshake', 'loudnorm', 'ebur128', 'scale', 'zscale'],
};

function run(...args) {
  const result = spawnSync(exe, ['-hide_banner', ...args], { encoding: 'utf8' });
  if (result.error) throw result.error;
  return `${result.stdout}${result.stderr}`;
}

/** Second column of `ffmpeg -encoders|-decoders|-filters` listings. */
function names(listing) {
  return new Set(
    listing
      .split(/\r?\n/)
      .map((line) => line.trim().split(/\s+/))
      // Flag columns: 6 characters for codecs ("V....D"), 2 for filters ("TS").
      .filter((cols) => cols.length >= 2 && /^[A-Z.|]{2,}$/.test(cols[0]))
      .map((cols) => cols[1]),
  );
}

if (!existsSync(exe)) {
  console.error(`No FFmpeg at ${exe}. Run \`pnpm run ffmpeg:fetch\` first.`);
  process.exit(2);
}

const failures = [];
const notes = [];

const versionLine = run('-version').split(/\r?\n/)[0];
if (!versionLine.includes(`n${lock.version}`)) {
  failures.push(`version mismatch: lock.json wants ${lock.version}, binary says "${versionLine}"`);
}

const flags = run('-buildconf')
  .split(/\s+/)
  .filter((token) => token.startsWith('--'));
for (const flag of ['--enable-gpl', '--enable-nonfree']) {
  if (flags.includes(flag)) failures.push(`build configured with ${flag}`);
}
if (!flags.includes('--enable-shared'))
  failures.push('not a shared build (--enable-shared missing)');

const externals = flags
  .filter((flag) => /^--enable-(lib|frei0r)/.test(flag))
  .map((flag) => flag.slice('--enable-'.length))
  .sort();
for (const lib of externals) {
  if (BANNED.includes(lib)) failures.push(`banned external library: ${lib}`);
  if (SHIP_BLOCKERS[lib]) {
    const message = `ship blocker ${lib}: ${SHIP_BLOCKERS[lib]}`;
    (ship ? failures : notes).push(message);
  }
}

const licence = run('-L');
if (
  !/GNU Lesser General Public License/.test(licence) ||
  /GNU General Public License as published/.test(licence)
) {
  failures.push('`ffmpeg -L` does not report the GNU LGPL');
}
const lgplVersion = /version (\d(?:\.\d)?) of the License/.exec(licence)?.[1] ?? 'unknown';

const have = {
  encoders: names(run('-encoders')),
  decoders: names(run('-decoders')),
  filters: names(run('-filters')),
  hwaccels: new Set(
    run('-hwaccels')
      .split(/\r?\n/)
      .slice(1)
      .map((s) => s.trim())
      .filter(Boolean),
  ),
};
const capabilities = Object.fromEntries(
  Object.entries(WANT).map(([kind, wanted]) => [
    kind,
    Object.fromEntries(wanted.map((name) => [name, have[kind].has(name)])),
  ]),
);

const bin = join(prefix, 'bin');
const dlls = (await readdir(bin)).filter((file) => file.toLowerCase().endsWith('.dll'));
let dllBytes = 0;
for (const dll of dlls) dllBytes += (await stat(join(bin, dll))).size;

const report = {
  ok: failures.length === 0,
  mode: ship ? 'ship' : 'development',
  version: versionLine,
  lgpl_version: lgplVersion,
  archive: { file: lock.file, sha256: lock.sha256, bytes: lock.bytes },
  dlls: { count: dlls.length, bytes: dllBytes, files: dlls.sort() },
  external_libraries: externals,
  capabilities,
  failures,
  notes,
};

if (jsonPath) await writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`);

const mb = (bytes) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;
console.log(versionLine);
console.log(`Licence: GNU LGPL v${lgplVersion}`);
console.log(`DLLs: ${dlls.length} files, ${mb(dllBytes)} (archive ${mb(lock.bytes)})`);
console.log(`External libraries (${externals.length}): ${externals.join(', ')}`);
for (const [kind, map] of Object.entries(capabilities)) {
  const missing = Object.entries(map)
    .filter(([, ok]) => !ok)
    .map(([name]) => name);
  console.log(
    `${kind}: ${Object.keys(map).length - missing.length}/${Object.keys(map).length}${missing.length ? ` (missing: ${missing.join(', ')})` : ''}`,
  );
}
for (const note of notes) console.log(`NOTE ${note}`);
for (const failure of failures) console.error(`FAIL ${failure}`);
const mode = ship ? 'ship' : 'development';
console.log(`FFmpeg licence check (${mode}): ${report.ok ? 'OK' : 'FAILED'}`);
process.exit(report.ok ? 0 : 1);
