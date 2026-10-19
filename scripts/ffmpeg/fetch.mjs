// Downloads the FFmpeg build pinned in scripts/ffmpeg/lock.json into
// vendor/ffmpeg-lgpl (the FFMPEG_DIR set by .cargo/config.toml), checks its
// size and SHA-256, then runs the licence verification.
//
// Usage: pnpm run ffmpeg:fetch [--force]
//
// Idempotent: an existing install with the same digest is kept.

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream, createWriteStream, existsSync } from 'node:fs';
import { mkdir, readdir, readFile, rename, rm, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const lock = JSON.parse(await readFile(join(root, 'scripts', 'ffmpeg', 'lock.json'), 'utf8'));
const vendor = join(root, 'vendor');
const target = join(vendor, 'ffmpeg-lgpl');
const marker = join(target, '.kinora-ffmpeg.json');
const archive = join(vendor, '.cache', lock.file);
const force = process.argv.includes('--force');

async function sha256(path) {
  const hash = createHash('sha256');
  await pipeline(createReadStream(path), hash);
  return hash.digest('hex');
}

async function isValid(path) {
  if (!existsSync(path)) return false;
  if ((await stat(path)).size !== lock.bytes) return false;
  return (await sha256(path)) === lock.sha256;
}

async function download() {
  await mkdir(dirname(archive), { recursive: true });
  for (const url of [...lock.mirrors, lock.source]) {
    console.log(`Downloading ${url}`);
    try {
      const response = await fetch(url, { redirect: 'follow' });
      if (!response.ok || !response.body) throw new Error(`HTTP ${response.status}`);
      await pipeline(Readable.fromWeb(response.body), createWriteStream(archive));
      if (await isValid(archive)) return;
      throw new Error('size or SHA-256 does not match lock.json');
    } catch (error) {
      console.warn(`  failed: ${error.message}`);
      await rm(archive, { force: true });
    }
  }
  throw new Error('No source delivered the pinned FFmpeg archive.');
}

async function extract() {
  const staging = join(vendor, '.cache', 'extract');
  await rm(staging, { recursive: true, force: true });
  await mkdir(staging, { recursive: true });
  // Windows' bundled bsdtar reads zip archives. It is called by full path: a
  // GNU tar earlier on PATH (Git Bash, MSYS2) reads "D:\..." as a remote host.
  const [command, args] =
    process.platform === 'win32'
      ? [
          join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe'),
          ['-xf', archive, '-C', staging],
        ]
      : ['unzip', ['-q', archive, '-d', staging]];
  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.status !== 0) throw new Error(`${command} exited with ${result.status}`);

  const entries = await readdir(staging);
  if (entries.length !== 1)
    throw new Error(`Expected one top-level folder, found ${entries.length}`);
  await rm(target, { recursive: true, force: true });
  await rename(join(staging, entries[0]), target);
  await rm(staging, { recursive: true, force: true });
}

async function installed() {
  if (!existsSync(marker)) return false;
  const current = JSON.parse(await readFile(marker, 'utf8'));
  return current.sha256 === lock.sha256;
}

if (!force && (await installed())) {
  console.log(`FFmpeg ${lock.version} (${lock.variant}) already installed in vendor/ffmpeg-lgpl.`);
} else {
  if (force || !(await isValid(archive))) await download();
  await extract();
  await writeFile(
    marker,
    `${JSON.stringify({ version: lock.version, file: lock.file, sha256: lock.sha256 }, null, 2)}\n`,
  );
  console.log(`Installed FFmpeg ${lock.version} (${lock.variant}) into vendor/ffmpeg-lgpl.`);
}

const verify = spawnSync(process.execPath, [join(root, 'scripts', 'ffmpeg', 'verify.mjs')], {
  stdio: 'inherit',
});
process.exit(verify.status ?? 1);
