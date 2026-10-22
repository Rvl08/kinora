// Shared helpers for the test media fixture scripts (fetch, generate,
// verify). The fixtures themselves are described in manifest.json; see
// README.md for the format and the verification rules.

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream, existsSync } from 'node:fs';
import { readFile, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { pipeline } from 'node:stream/promises';
import { fileURLToPath } from 'node:url';

export const root = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
export const manifestPath = join(root, 'scripts', 'fixtures', 'manifest.json');
export const outDir = join(root, 'tests', 'media', 'fixtures');
export const sourcesDir = join(outDir, '.sources');
export const tmpDir = join(outDir, '.tmp');
const statePath = join(outDir, '.state.json');

const bin = join(root, 'vendor', 'ffmpeg-lgpl', 'bin');
const exe = process.platform === 'win32' ? '.exe' : '';
export const ffmpeg = join(bin, `ffmpeg${exe}`);
export const ffprobe = join(bin, `ffprobe${exe}`);

/** Hardware encoders: output depends on the GPU and driver, so hashes are advisory. */
export const isHardwareEncoder = (name) => /_(nvenc|qsv|amf|mf|vaapi|vulkan|d3d12va)$/.test(name);

export function requireFfmpeg() {
  if (!existsSync(ffmpeg) || !existsSync(ffprobe)) {
    console.error(`No FFmpeg in ${bin}. Run \`pnpm run ffmpeg:fetch\` first.`);
    process.exit(2);
  }
}

/** Value of `--name value`, or undefined. */
export function option(name) {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : undefined;
}

export const flag = (name) => process.argv.includes(`--${name}`);

export async function loadManifest() {
  return JSON.parse(await readFile(manifestPath, 'utf8'));
}

/** Fixtures of `kind` (or all), narrowed by `--ids a,b`. Unknown ids are an error. */
export function selectFixtures(manifest, kind) {
  const ids = option('ids')?.split(',').filter(Boolean);
  for (const id of ids ?? []) {
    if (!manifest.fixtures.some((f) => f.id === id)) throw new Error(`unknown fixture id: ${id}`);
  }
  return manifest.fixtures.filter(
    (f) => (!kind || f.kind === kind) && (!ids || ids.includes(f.id)),
  );
}

export const fixtureFile = (file) => join(outDir, ...file.path.split('/'));

export async function sha256File(path) {
  const hash = createHash('sha256');
  await pipeline(createReadStream(path), hash);
  return hash.digest('hex');
}

/** Runs a tool to completion; throws with its stderr on failure. */
export function run(command, args) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const tail = `${result.stderr ?? ''}`.trim().split(/\r?\n/).slice(-15).join('\n');
    throw new Error(`${command} exited with ${result.status}\n${tail}`);
  }
  return result.stdout;
}

/** The FFmpeg build string, e.g. "n8.1.3-18-g46d899d7a0" (date suffix dropped). */
export function ffmpegBuild() {
  const line = run(ffmpeg, ['-hide_banner', '-version']).split(/\r?\n/)[0];
  const version = /^ffmpeg version (\S+)/.exec(line)?.[1] ?? 'unknown';
  return version.replace(/-\d{8}$/, '');
}

// Per-machine record of what produced each local file (FFmpeg build,
// encoder, hashes). It lets verify tell "different build" from "corrupt".
export async function loadState() {
  if (!existsSync(statePath)) return {};
  return JSON.parse(await readFile(statePath, 'utf8'));
}

export async function saveState(state) {
  await writeFile(statePath, `${JSON.stringify(state, null, 2)}\n`);
}

/**
 * Whether every file of a fixture is present and matches either its pinned
 * hash or the hash this machine recorded when it last produced it with the
 * current FFmpeg build.
 */
export async function isCurrent(fixture, state, build) {
  const record = state[fixture.id];
  for (const file of fixture.files) {
    const path = fixtureFile(file);
    if (!existsSync(path)) return false;
    const digest = await sha256File(path);
    if (digest === file.sha256) continue;
    if (record?.ffmpeg === build && record.files?.[file.path] === digest) continue;
    return false;
  }
  return true;
}

/** Records the local result of fetch or generate in the state file. */
export async function recordState(state, fixture, build, encoder) {
  const files = {};
  for (const file of fixture.files) files[file.path] = await sha256File(fixtureFile(file));
  state[fixture.id] = {
    ffmpeg: build,
    encoder: encoder ?? null,
    files,
    at: new Date().toISOString(),
  };
  await saveState(state);
  return files;
}

/**
 * Writes the hashes and sizes of the local files into the manifest (`--pin`).
 * Only for maintainers updating the reference set; review the diff.
 */
export async function pin(manifest, fixture, build, encoder) {
  for (const file of fixture.files) {
    const path = fixtureFile(file);
    file.bytes = (await stat(path)).size;
    file.sha256 = await sha256File(path);
  }
  fixture.pin = { ffmpeg: build, ...(encoder ? { encoder } : {}) };
  let text = `${JSON.stringify(manifest, null, 2)}\n`;
  try {
    const prettier = await import('prettier');
    const config = (await prettier.resolveConfig(manifestPath)) ?? {};
    text = await prettier.format(text, { ...config, filepath: manifestPath });
  } catch {
    console.warn('  prettier not available: run `pnpm run format` before committing');
  }
  await writeFile(manifestPath, text);
}

const rate = (s) => {
  const [n, d] = `${s}`.split('/').map(Number);
  return d ? n / d : n;
};

function probe(path, extra = []) {
  return JSON.parse(run(ffprobe, ['-v', 'error', '-of', 'json', ...extra, path]));
}

const within = (value, [lo, hi]) => value >= lo && value <= hi;

/**
 * Checks a file against the `expect` block of its fixture with ffprobe and
 * returns a list of problems (empty when it matches).
 */
export function checkProperties(path, expect) {
  const problems = [];
  const info = probe(path, ['-show_format', '-show_streams', '-show_stream_groups']);
  const same = (what, actual, wanted) => {
    if (wanted !== undefined && `${actual}` !== `${wanted}`)
      problems.push(`${what} ${actual}, want ${wanted}`);
  };
  same('format', info.format.format_name, expect.format);
  if (expect.duration) {
    const duration = Number(info.format.duration);
    if (!within(duration, expect.duration))
      problems.push(`duration ${duration}, want ${expect.duration.join('..')}`);
  }

  const streams = info.streams.filter((s) => !s.disposition?.attached_pic);
  for (const type of ['video', 'audio']) {
    const want = expect[type];
    const stream = streams.find((s) => s.codec_type === type);
    if (want === false) {
      if (stream) problems.push(`unexpected ${type} stream`);
      continue;
    }
    if (!want) continue;
    if (!stream) {
      problems.push(`no ${type} stream`);
      continue;
    }
    same(`${type} codec`, stream.codec_name, want.codec);
    same(`${type} profile`, stream.profile, want.profile);
    same('width', stream.width, want.width);
    same('height', stream.height, want.height);
    same('pix_fmt', stream.pix_fmt, want.pixFmt);
    same('sample rate', stream.sample_rate, want.sampleRate);
    same('channels', stream.channels, want.channels);
    if (want.fps) {
      const fps = rate(stream.avg_frame_rate);
      if (!within(fps, want.fps))
        problems.push(`avg fps ${fps.toFixed(3)}, want ${want.fps.join('..')}`);
    }
    if (want.color) {
      same('color primaries', stream.color_primaries, want.color.primaries);
      same('color transfer', stream.color_transfer, want.color.transfer);
      same('color space', stream.color_space, want.color.space);
      same('color range', stream.color_range, want.color.range);
    }
    if (want.alpha !== undefined) {
      const alpha =
        stream.tags?.alpha_mode === '1' || /^(yuva|gbra|rgba|bgra|argb|abgr)/.test(stream.pix_fmt);
      if (alpha !== want.alpha) problems.push(`alpha ${alpha}, want ${want.alpha}`);
    }
    if (want.hdr10) checkHdr10(stream, want.hdr10, problems);
    if (want.vfr !== undefined) checkVfr(path, want.vfr, problems);
    if (want.rotation !== undefined || want.icc !== undefined) checkFrame(path, want, problems);
  }

  if (expect.tileGrid) {
    const grid = info.stream_groups?.find((g) => g.type === 'Tile Grid');
    const tile =
      grid?.components?.find((c) => c.component_type === 'tile_grid') ?? grid?.components?.[0];
    if (!grid) problems.push('no tile grid');
    else {
      same('grid width', tile?.width, expect.tileGrid.width);
      same('grid height', tile?.height, expect.tileGrid.height);
      same('grid tiles', grid.nb_streams, expect.tileGrid.tiles);
    }
  }
  return problems;
}

function checkHdr10(stream, want, problems) {
  const side = stream.side_data_list ?? [];
  const mastering = side.find((d) => d.side_data_type === 'Mastering display metadata');
  const light = side.find((d) => d.side_data_type === 'Content light level metadata');
  if (!mastering) problems.push('no mastering display metadata');
  if (!light) problems.push('no content light level metadata');
  else if (light.max_content !== want.maxCll || light.max_average !== want.maxFall) {
    problems.push(
      `MaxCLL/MaxFALL ${light.max_content}/${light.max_average}, want ${want.maxCll}/${want.maxFall}`,
    );
  }
}

/** VFR = more than one distinct frame duration (1 ms tolerance). */
function checkVfr(path, want, problems) {
  const packets = probe(path, [
    '-select_streams',
    'v:0',
    '-show_entries',
    'packet=pts_time',
  ]).packets;
  const times = packets.map((p) => Number(p.pts_time)).sort((a, b) => a - b);
  const steps = [];
  for (let i = 1; i < times.length; i++) {
    const step = times[i] - times[i - 1];
    if (!steps.some((s) => Math.abs(s - step) < 0.001)) steps.push(step);
  }
  const vfr = steps.length > 1;
  if (vfr !== want)
    problems.push(`vfr ${vfr} (${steps.length} distinct frame durations), want ${want}`);
}

/** Rotation (EXIF/display matrix) and ICC profile, from the first decoded frame. */
function checkFrame(path, want, problems) {
  const frame = probe(path, ['-select_streams', 'v:0', '-read_intervals', '%+#1', '-show_frames'])
    .frames?.[0];
  const side = frame?.side_data_list ?? [];
  if (want.rotation !== undefined) {
    const matrix = side.find((d) => /display ?matrix/i.test(d.side_data_type));
    const rotation = matrix?.rotation ?? 0;
    if (rotation !== want.rotation) problems.push(`rotation ${rotation}, want ${want.rotation}`);
  }
  if (want.icc !== undefined) {
    const icc = side.some((d) => d.side_data_type === 'ICC profile');
    if (icc !== want.icc) problems.push(`ICC profile ${icc}, want ${want.icc}`);
  }
}

export function formatBytes(bytes) {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(2)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  return `${(bytes / 1e3).toFixed(0)} kB`;
}

/** Prints rows (arrays of strings) as an aligned plain-text table. */
export function printTable(header, rows) {
  const widths = header.map((h, i) => Math.max(h.length, ...rows.map((r) => `${r[i]}`.length)));
  const line = (cells) =>
    cells
      .map((c, i) => `${c}`.padEnd(widths[i]))
      .join('  ')
      .trimEnd();
  console.log(line(header));
  console.log(line(widths.map((w) => '-'.repeat(w))));
  for (const row of rows) console.log(line(row));
}
