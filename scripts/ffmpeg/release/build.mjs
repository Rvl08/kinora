// Builds Kinora's release FFmpeg (P0-05b, ADR-0005 Amendment 3): static
// libraries and headers for x86_64-pc-windows-msvc, linked into kinora.exe,
// containing exactly the components in scripts/ffmpeg/release-components.json.
//
// Usage (Windows, from the repository root):
//   node scripts/ffmpeg/release/build.mjs [--work C:\kf] [--cache <dir>] [--out <dir>]
//                                         [--dist <dir>] [--jobs <n>] [--skip-verify]
//                                         [--source <name>=<dir> ...]
//
//   --work   scratch directory; keep it short, MSBuild and CMake break on long paths.
//            Default C:\kf. Wiped at the start of every build.
//   --cache  downloads and unpacked tools, reused between builds.
//            Default vendor/.cache/ffmpeg-release.
//   --tools  where the downloaded tools are unpacked. Default <cache>/tools. Windows
//            Sandbox passes a VM-local folder: executables in its mapped folders
//            fail to start intermittently.
//   --out    the installed artifact (FFMPEG_DIR for Rust). Default vendor/ffmpeg-static.
//   --dist   also write <name>.zip, <name>.zip.sha256 and sources/ (every pinned
//            source archive, for the LGPL "equivalent access" to source).
//   --source use an unpacked source tree instead of the pinned archive, e.g.
//            --source ffmpeg=C:\src\ffmpeg-modified (LGPL relinking, see
//            scripts/ffmpeg/RELINKING.md). The manifest records it as local.
//
// The same script runs in GitHub Actions (.github/workflows/ffmpeg-release.yml)
// and in Windows Sandbox (scripts/ffmpeg/release/sandbox-job.ps1). It needs
// MSVC (found through vswhere when the environment has no vcvars), CMake and
// Node; everything else is downloaded from release-sources.json and checked
// against its SHA-256 before use.
//
// CRT: every library is compiled with the static CRT (/MT, LIBCMT), matching
// Rust's `-C target-feature=+crt-static` (ADR-0006 Amendment 1). Mixing /MT
// and /MD objects in one link is an error or a silent double CRT, so the
// release gate rejects any object that asks for MSVCRT.

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream, createWriteStream, existsSync } from 'node:fs';
import {
  copyFile,
  cp,
  mkdir,
  readdir,
  readFile,
  rename,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises';
import { availableParallelism } from 'node:os';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { fileURLToPath } from 'node:url';

import { configureFlags, loadSpec } from './components.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '..', '..', '..');
const lock = JSON.parse(
  await readFile(join(root, 'scripts', 'ffmpeg', 'release-sources.json'), 'utf8'),
);
const spec = await loadSpec(root);

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  return index === -1 ? fallback : process.argv[index + 1];
}

const work = resolve(option('--work', 'C:\\kf'));
const cache = resolve(option('--cache', join(root, 'vendor', '.cache', 'ffmpeg-release')));
const out = resolve(option('--out', join(root, 'vendor', 'ffmpeg-static')));
const dist = option('--dist') ? resolve(option('--dist')) : null;
const jobs = Number(option('--jobs', availableParallelism()));
const overrides = Object.fromEntries(
  process.argv
    .flatMap((arg, i) => (arg === '--source' ? [process.argv[i + 1] ?? ''] : []))
    .map((value) => {
      const at = value.indexOf('=');
      if (at < 1) throw new Error(`--source expects <name>=<dir>, got "${value}"`);
      return [value.slice(0, at), resolve(value.slice(at + 1))];
    }),
);
for (const name of Object.keys(overrides)) {
  if (!lock.sources.some((s) => s.name === name))
    throw new Error(`--source: unknown source ${name}`);
}
const downloads = join(cache, 'downloads');
const tools = resolve(option('--tools', join(cache, 'tools')));
const srcRoot = join(work, 's');
const buildRoot = join(work, 'b');
const prefix = join(work, 'p');
const fwd = (path) => path.replaceAll('\\', '/');
const posix = (path) => fwd(path).replace(/^([A-Za-z]):/, (_, d) => `/${d.toLowerCase()}`);
const ffmpegSource = lock.sources.find((s) => s.name === 'ffmpeg');
const artifactName = `ffmpeg-${ffmpegSource.version}-r${lock.revision}-win64-lgpl-static`;

if (process.platform !== 'win32') {
  console.error('The release FFmpeg targets x86_64-pc-windows-msvc and builds on Windows only.');
  process.exit(2);
}

// ---------------------------------------------------------------- helpers

const timings = [];
async function step(name, fn) {
  console.log(`\n=== ${name}`);
  const start = Date.now();
  const result = await fn();
  const seconds = Math.round((Date.now() - start) / 1000);
  timings.push({ step: name, seconds });
  console.log(`=== ${name}: ${seconds} s`);
  return result;
}

/** Environment with PATH entries prepended, whatever case the key has. */
function withPath(env, ...entries) {
  const key = Object.keys(env).find((k) => k.toUpperCase() === 'PATH') ?? 'PATH';
  const next = { ...env };
  delete next[key];
  next.PATH = [...entries, env[key] ?? ''].join(';');
  return next;
}

function run(command, args, { cwd = work, env = buildEnv, capture = false } = {}) {
  const shown = [basename(command), ...args].join(' ');
  console.log(`> ${shown.length > 400 ? `${shown.slice(0, 400)} ...` : shown}`);
  const result = spawnSync(command, args, {
    cwd,
    env,
    stdio: capture ? ['ignore', 'pipe', 'inherit'] : 'inherit',
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
    windowsVerbatimArguments: false,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${basename(command)} exited with ${result.status}`);
  return capture ? result.stdout : '';
}

async function sha256(path) {
  const hash = createHash('sha256');
  await pipeline(createReadStream(path), hash);
  return hash.digest('hex');
}

/** Downloads `entry` into the cache unless a verified copy is there already. */
async function fetchPinned(entry, file, urls) {
  const target = join(downloads, file);
  if (existsSync(target) && (await sha256(target)) === entry.sha256) return target;
  if (!/^[0-9a-f]{64}$/.test(entry.sha256 ?? '')) throw new Error(`${file}: no SHA-256 pinned`);
  await mkdir(downloads, { recursive: true });
  const partial = `${target}.part`;
  for (const url of urls) {
    for (let attempt = 1; attempt <= 3; attempt++) {
      console.log(`download ${url}${attempt > 1 ? ` (attempt ${attempt})` : ''}`);
      try {
        const response = await fetch(url, {
          redirect: 'follow',
          signal: AbortSignal.timeout(20 * 60 * 1000),
        });
        if (!response.ok || !response.body) throw new Error(`HTTP ${response.status}`);
        await pipeline(Readable.fromWeb(response.body), createWriteStream(partial));
        const digest = await sha256(partial);
        if (digest !== entry.sha256) {
          // A wrong digest is not a network glitch: do not retry this URL.
          await rm(partial, { force: true });
          console.warn(`  SHA-256 mismatch: got ${digest}, pinned ${entry.sha256}`);
          break;
        }
        await rename(partial, target);
        return target;
      } catch (error) {
        console.warn(`  failed: ${error.message}`);
        await rm(partial, { force: true });
      }
    }
  }
  throw new Error(`${file}: no URL delivered the pinned archive (${entry.sha256})`);
}

const archiveName = (entry) => entry.file ?? basename(new URL(entry.url).pathname);
const entryUrls = (entry) => [...(entry.mirrors ?? []), entry.url];

const systemTar = join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe');

/**
 * Unpacks an archive into a fresh `dir` and returns the single top-level
 * folder inside it (or `dir` when the archive has several). Windows' bsdtar
 * reads tar.gz/xz/zst and zip; a GNU tar earlier on PATH would read "C:\..." as
 * a remote host, hence the full path.
 */
async function unpack(archive, dir) {
  await rm(dir, { recursive: true, force: true });
  await mkdir(dir, { recursive: true });
  run(systemTar, ['-xf', archive, '-C', dir], { env: process.env });
  const entries = await readdir(dir, { withFileTypes: true });
  return entries.length === 1 && entries[0].isDirectory() ? join(dir, entries[0].name) : dir;
}

async function files(dir, filter = () => true) {
  const found = [];
  for (const entry of await readdir(dir, { withFileTypes: true, recursive: true })) {
    if (entry.isFile() && filter(entry.name)) found.push(join(entry.parentPath, entry.name));
  }
  return found;
}

// ---------------------------------------------------------------- toolchain

/** Imports the vcvars64 environment unless the caller already did. */
function msvcEnvironment() {
  if (process.env.VCToolsInstallDir && process.env.INCLUDE && process.env.LIB) return process.env;
  const vswhere = join(
    process.env['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)',
    'Microsoft Visual Studio',
    'Installer',
    'vswhere.exe',
  );
  if (!existsSync(vswhere)) throw new Error('MSVC not found: run from a VS developer prompt.');
  const vs = spawnSync(
    vswhere,
    [
      '-latest',
      '-products',
      '*',
      '-requires',
      'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
      '-property',
      'installationPath',
    ],
    { encoding: 'utf8' },
  ).stdout.trim();
  if (!vs) throw new Error('No Visual Studio with the x64 C++ tools.');
  const vcvars = join(vs, 'VC', 'Auxiliary', 'Build', 'vcvars64.bat');
  const dump = spawnSync('cmd.exe', ['/d', '/s', '/c', `""${vcvars}" >nul && set"`], {
    encoding: 'utf8',
    windowsVerbatimArguments: true,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (dump.status !== 0) throw new Error(`vcvars64.bat failed: ${dump.stderr}`);
  const env = {};
  for (const line of dump.stdout.split(/\r?\n/)) {
    const at = line.indexOf('=');
    if (at > 0) env[line.slice(0, at)] = line.slice(at + 1);
  }
  return env;
}

const msvcEnv = msvcEnvironment();
// Everything below compiles with cl.exe; make sure nothing picks gcc or clang
// from PATH (GitHub's Windows image has both).
const baseEnv = { ...msvcEnv, CC: 'cl', CXX: 'cl', CMAKE_GENERATOR: 'Ninja' };
for (const key of ['CFLAGS', 'CXXFLAGS', 'LDFLAGS', 'CL', '_CL_', 'LINK', '_LINK_'])
  delete baseEnv[key];
let buildEnv = baseEnv;

const vsMajor = Number.parseInt(msvcEnv.VisualStudioVersion ?? '0', 10);
const toolsVersion = msvcEnv.VCToolsVersion ?? 'unknown';

const tool = {};

async function setupTools() {
  await mkdir(tools, { recursive: true });
  const ready = async (dir, sha) =>
    existsSync(join(dir, '.kinora-ready')) &&
    (await readFile(join(dir, '.kinora-ready'), 'utf8')) === sha;
  const markReady = (dir, sha) => writeFile(join(dir, '.kinora-ready'), sha);

  for (const entry of lock.tools) {
    const archive = await fetchPinned(entry, archiveName(entry), entryUrls(entry));
    if (entry.name === 'msys2') continue; // unpacked below, together with its packages
    const dir = join(tools, `${entry.name}-${entry.version}`);
    if (await ready(dir, entry.sha256)) {
      tool[entry.name] = dir;
      continue;
    }
    await rm(dir, { recursive: true, force: true });
    if (entry.name === 'meson') {
      // Administrative install = unpack only: nothing is registered on the machine.
      await mkdir(dir, { recursive: true });
      run('msiexec.exe', ['/a', archive, '/qn', `TARGETDIR=${dir}`], { env: process.env });
    } else {
      const top = await unpack(archive, `${dir}.tmp`);
      await rename(top, dir);
      await rm(`${dir}.tmp`, { recursive: true, force: true });
    }
    await markReady(dir, entry.sha256);
    tool[entry.name] = dir;
  }

  const [nasm] = await files(tool.nasm, (n) => n.toLowerCase() === 'nasm.exe');
  const [meson] = await files(tool.meson, (n) => n.toLowerCase() === 'meson.exe');
  const [ninja] = await files(tool.ninja, (n) => n.toLowerCase() === 'ninja.exe');
  if (!nasm || !meson || !ninja) throw new Error('nasm, meson or ninja missing after unpacking');
  tool.nasmExe = nasm;
  tool.mesonExe = meson;
  tool.ninjaExe = ninja;

  // MSYS2: pinned base plus the pinned packages it lacks, installed offline.
  const msysEntry = lock.tools.find((t) => t.name === 'msys2');
  const packages = lock.msys2Packages;
  const stamp = [msysEntry.sha256, ...packages.files.map((f) => f.sha256)].join(',');
  const msys = join(tools, 'msys64');
  tool.msys = msys;
  if (!(await ready(msys, stamp))) {
    await rm(msys, { recursive: true, force: true });
    const top = await unpack(join(downloads, archiveName(msysEntry)), `${msys}.tmp`);
    await rename(top, msys);
    await rm(`${msys}.tmp`, { recursive: true, force: true });
    const pkgFiles = [];
    for (const pkg of packages.files) {
      const urls = [...packages.mirrors, packages.baseUrl].map((base) => `${base}${pkg.file}`);
      pkgFiles.push(fwd(await fetchPinned(pkg, pkg.file, urls)));
    }
    // Unpacked straight into the root with MSYS2's own bsdtar (it keeps MSYS
    // symlinks) instead of `pacman -U`: pacman insists on a populated GnuPG
    // keyring to read the base's signed repository databases, which an offline,
    // throwaway toolchain does not need. The SHA-256 pins above are the
    // integrity check, and the list is already the full dependency closure.
    const metadata = ['.PKGINFO', '.MTREE', '.BUILDINFO', '.INSTALL'];
    bash(
      pkgFiles
        .map((f) => `bsdtar -xpf '${f}' -C / ${metadata.map((m) => `--exclude '${m}'`).join(' ')}`)
        .join(' && '),
      { cwd: work },
    );
    await markReady(msys, stamp);
  }

  buildEnv = withPath(
    baseEnv,
    dirname(tool.nasmExe),
    dirname(tool.ninjaExe),
    dirname(tool.mesonExe),
  );
}

/** Runs a script in MSYS2's bash (non-login: the MSVC environment is kept). */
function bash(script, { cwd }) {
  const env = withPath(buildEnv, join(tool.msys, 'usr', 'bin'));
  env.MSYSTEM = 'MSYS';
  env.CHERE_INVOKING = '1';
  return run(
    join(tool.msys, 'usr', 'bin', 'bash.exe'),
    ['--noprofile', '--norc', '-e', '-o', 'pipefail', '-c', script],
    { cwd, env },
  );
}

// ---------------------------------------------------------------- libraries

const sourceDir = {};

async function fetchSources() {
  for (const entry of lock.sources) {
    if (overrides[entry.name]) {
      console.warn(
        `${entry.name}: local source ${overrides[entry.name]} instead of the pinned archive`,
      );
      sourceDir[entry.name] = overrides[entry.name];
      continue;
    }
    const archive = await fetchPinned(entry, archiveName(entry), entryUrls(entry));
    sourceDir[entry.name] = await unpack(archive, join(srcRoot, entry.name));
  }
}

function cmake(name, source, defines) {
  const dir = join(buildRoot, name);
  run('cmake', [
    '-S',
    source,
    '-B',
    dir,
    '-G',
    'Ninja',
    `-DCMAKE_MAKE_PROGRAM=${fwd(tool.ninjaExe)}`,
    '-DCMAKE_BUILD_TYPE=Release',
    '-DCMAKE_C_COMPILER=cl',
    '-DCMAKE_CXX_COMPILER=cl',
    `-DCMAKE_ASM_NASM_COMPILER=${fwd(tool.nasmExe)}`,
    `-DCMAKE_INSTALL_PREFIX=${fwd(prefix)}`,
    '-DCMAKE_INSTALL_LIBDIR=lib',
    '-DBUILD_SHARED_LIBS=OFF',
    // Static CRT (/MT) everywhere; CMP0091 makes CMAKE_MSVC_RUNTIME_LIBRARY
    // apply to projects that still declare an old minimum version.
    '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded',
    '-DCMAKE_POLICY_DEFAULT_CMP0091=NEW',
    '-DCMAKE_POLICY_VERSION_MINIMUM=3.5',
    ...defines,
  ]);
  run('cmake', ['--build', dir, '--parallel', String(jobs)]);
  run('cmake', ['--install', dir]);
}

const builders = {
  zlib: () =>
    cmake('zlib', sourceDir.zlib, [
      '-DZLIB_BUILD_SHARED=OFF',
      '-DZLIB_BUILD_STATIC=ON',
      '-DZLIB_BUILD_TESTING=OFF',
    ]),

  opus: () =>
    cmake('opus', sourceDir.opus, [
      '-DOPUS_STATIC_RUNTIME=ON',
      '-DOPUS_BUILD_PROGRAMS=OFF',
      '-DOPUS_BUILD_TESTING=OFF',
      '-DOPUS_INSTALL_PKG_CONFIG_MODULE=ON',
      '-DOPUS_INSTALL_CMAKE_CONFIG_MODULE=OFF',
    ]),

  lame: () => cmake('lame', join(here, 'lame'), [`-DLAME_SOURCE_DIR=${fwd(sourceDir.lame)}`]),

  'svt-av1': () =>
    cmake('svt-av1', sourceDir['svt-av1'], [
      '-DBUILD_APPS=OFF',
      '-DBUILD_TESTING=OFF',
      // LTO would put /GL objects in the static library: every consumer would
      // then need link-time code generation, and the gate could not read them.
      '-DSVT_AV1_LTO=OFF',
      '-DEXCLUDE_HASH=ON',
      '-DREPRODUCIBLE_BUILDS=ON',
    ]),

  libvpl: () =>
    cmake('libvpl', sourceDir.libvpl, [
      '-DUSE_MSVC_STATIC_RUNTIME=ON',
      '-DBUILD_TESTS=OFF',
      '-DBUILD_EXAMPLES=OFF',
      '-DINSTALL_EXAMPLES=OFF',
      '-DBUILD_EXPERIMENTAL=OFF',
    ]),

  dav1d: () => {
    const dir = join(buildRoot, 'dav1d');
    run(tool.mesonExe, [
      'setup',
      dir,
      sourceDir.dav1d,
      '--buildtype=release',
      '--default-library=static',
      '-Db_vscrt=mt',
      '-Db_ndebug=true',
      `--prefix=${fwd(prefix)}`,
      '--libdir=lib',
      '-Denable_tools=false',
      '-Denable_tests=false',
      '-Denable_examples=false',
      '-Denable_docs=false',
    ]);
    run(tool.mesonExe, ['compile', '-C', dir, '-j', String(jobs)]);
    run(tool.mesonExe, ['install', '-C', dir]);
  },

  libvpx: async () => {
    // libvpx builds with MSVC only through generated Visual Studio projects:
    // configure + make (MSYS2, needs perl for the RTCD headers) write them,
    // MSBuild compiles them.
    if (vsMajor < 17) throw new Error(`libvpx needs Visual Studio 2022 or later, found ${vsMajor}`);
    const target = `x86_64-win64-vs${Math.min(vsMajor, 18)}`;
    const dir = join(buildRoot, 'libvpx');
    await mkdir(dir, { recursive: true });
    // With CC unset, configure takes the "external build" path for Visual
    // Studio targets; with CC=cl it would try to link test programs through
    // MSYS2's coreutils `link`, which fails.
    bash(
      [
        'unset CC CXX LD AR AS &&',
        `'${posix(sourceDir.libvpx)}/configure'`,
        `--target=${target}`,
        '--as=nasm',
        '--enable-static-msvcrt',
        '--enable-static',
        '--disable-shared',
        '--disable-vp8',
        '--disable-vp9-decoder',
        '--enable-vp9-highbitdepth',
        '--disable-examples',
        '--disable-tools',
        '--disable-docs',
        '--disable-unit-tests',
        '--disable-libyuv',
        '--disable-webm-io',
        `&& make -j${jobs}`,
      ].join(' '),
      { cwd: dir },
    );
    run('msbuild', [
      join(dir, 'vpx.vcxproj'),
      '/nologo',
      '/m',
      '/p:Configuration=Release',
      '/p:Platform=x64',
      '/p:UseEnv=true',
      '/v:minimal',
    ]);
    const [lib] = await files(dir, (n) => /^vpxmt\.lib$/i.test(n));
    if (!lib) throw new Error('libvpx: vpxmt.lib not produced');
    await mkdir(join(prefix, 'lib', 'pkgconfig'), { recursive: true });
    await copyFile(lib, join(prefix, 'lib', 'vpx.lib'));
    const headers = join(prefix, 'include', 'vpx');
    await mkdir(headers, { recursive: true });
    for (const header of await readdir(join(sourceDir.libvpx, 'vpx'))) {
      if (header.endsWith('.h'))
        await copyFile(join(sourceDir.libvpx, 'vpx', header), join(headers, header));
    }
    const version = lock.sources.find((s) => s.name === 'libvpx').version;
    await writeFile(
      join(prefix, 'lib', 'pkgconfig', 'vpx.pc'),
      pc('vpx', 'WebM Project VPx codec implementation', version, '-lvpx'),
    );
  },

  'nv-codec-headers': async () => {
    // Its Makefile only copies headers and fills in the .pc file.
    const source = sourceDir['nv-codec-headers'];
    await cp(join(source, 'include', 'ffnvcodec'), join(prefix, 'include', 'ffnvcodec'), {
      recursive: true,
    });
    const template = await readFile(join(source, 'ffnvcodec.pc.in'), 'utf8');
    await mkdir(join(prefix, 'lib', 'pkgconfig'), { recursive: true });
    await writeFile(
      join(prefix, 'lib', 'pkgconfig', 'ffnvcodec.pc'),
      template.replaceAll('@@PREFIX@@', fwd(prefix)),
    );
  },

  'amf-headers': () =>
    cp(join(sourceDir['amf-headers'], 'AMF'), join(prefix, 'include', 'AMF'), { recursive: true }),
};

function pc(name, description, version, libs) {
  return [
    `prefix=${fwd(prefix)}`,
    'libdir=${prefix}/lib',
    'includedir=${prefix}/include',
    '',
    `Name: ${name}`,
    `Description: ${description}`,
    `Version: ${version}`,
    `Libs: -L\${libdir} ${libs}`,
    'Cflags: -I${includedir}',
    '',
  ].join('\n');
}

/**
 * Normalises the installed dependencies for FFmpeg's MSVC configure: static
 * libraries as <name>.lib (configure turns -lfoo into foo.lib, and -lz into
 * zlib.lib), no import libraries or DLLs, no Unix-only link flags in .pc files.
 */
async function normaliseDependencies() {
  const lib = join(prefix, 'lib');
  const renames = { 'libdav1d.a': 'dav1d.lib', 'zs.lib': 'zlib.lib' };
  for (const [from, to] of Object.entries(renames)) {
    if (existsSync(join(lib, from))) await rename(join(lib, from), join(lib, to));
  }
  await rm(join(prefix, 'bin'), { recursive: true, force: true });
  for (const file of await files(join(prefix, 'lib', 'pkgconfig'), (n) => n.endsWith('.pc'))) {
    const text = await readFile(file, 'utf8');
    const fixed = text
      .split(/\r?\n/)
      .map((line) => {
        if (/^prefix=/.test(line)) return `prefix=${fwd(prefix)}`;
        if (/^Libs(\.private)?:/.test(line)) {
          return line.replace(/\s-l(m|pthread|stdc\+\+|c\+\+|dl|rt)\b/g, '');
        }
        return line;
      })
      .join('\n');
    await writeFile(file, fixed);
  }
}

// ---------------------------------------------------------------- FFmpeg

let configureLine = '';

async function buildFfmpeg() {
  const dir = join(buildRoot, 'ffmpeg');
  await mkdir(dir, { recursive: true });
  const flags = [
    `--prefix=${posix(prefix)}`,
    '--toolchain=msvc',
    '--arch=x86_64',
    '--target-os=win64',
    '--pkg-config=pkgconf',
    '--pkg-config-flags=--static',
    '--enable-static',
    '--disable-shared',
    '--disable-programs',
    '--disable-doc',
    '--disable-debug',
    '--disable-network',
    '--disable-avdevice',
    // Nothing is picked up from the build machine: every feature below is
    // named, so two machines produce the same configuration.
    '--disable-autodetect',
    '--disable-everything',
    ...configureFlags(spec),
    `--extra-cflags=-MT -I${fwd(prefix)}/include`,
    // configure rewrites -L into link.exe's -libpath:.
    `--extra-ldflags=-L${fwd(prefix)}/lib`,
  ];
  for (const flag of flags) {
    if (/^--enable-(gpl|nonfree|version3)$/.test(flag)) throw new Error(`refusing ${flag}`);
  }
  configureLine = flags.join(' ');
  const quoted = flags.map((f) => `'${f.replaceAll("'", "'\\''")}'`).join(' ');
  bash(
    [
      `export PKG_CONFIG_PATH='${posix(join(prefix, 'lib', 'pkgconfig'))}'`,
      `'${posix(sourceDir.ffmpeg)}/configure' ${quoted} | tee configure.out`,
    ].join('\n'),
    { cwd: dir },
  );
  // configure only warns about component names it does not know; for the
  // release that is an error (the JSON and the build would disagree).
  const warnings = (await readFile(join(dir, 'configure.out'), 'utf8'))
    .split(/\r?\n/)
    .filter((line) => /^WARNING:/i.test(line));
  if (warnings.length) throw new Error(`configure warnings:\n${warnings.join('\n')}`);
  bash(`make -j${jobs} && make install`, { cwd: dir });

  // MSVC static libraries are named libavcodec.a by FFmpeg's build; Rust
  // (ffmpeg-sys-next) and link.exe expect avcodec.lib.
  for (const name of await readdir(join(prefix, 'lib'))) {
    const match = /^lib(av\w+|sw\w+)\.a$/.exec(name);
    if (match) await rename(join(prefix, 'lib', name), join(prefix, 'lib', `${match[1]}.lib`));
  }
  return dir;
}

/** Libraries the final link needs besides FFmpeg's own, from config.mak. */
async function linkLibraries(ffmpegBuild) {
  const mak = await readFile(join(ffmpegBuild, 'ffbuild', 'config.mak'), 'utf8');
  const names = [];
  for (const line of mak.split(/\r?\n/)) {
    if (!/^EXTRALIBS(-\w+)?=/.test(line)) continue;
    for (const token of line
      .slice(line.indexOf('=') + 1)
      .trim()
      .split(/\s+/)) {
      const name = /^-l(.+)$/.exec(token)?.[1] ?? /^([^-/:][^/:]*)\.lib$/i.exec(token)?.[1];
      if (name && !names.includes(name)) names.push(name);
    }
  }
  const staticLibs = [];
  const system = [];
  for (const name of names) {
    (existsSync(join(prefix, 'lib', `${name}.lib`)) ? staticLibs : system).push(name);
  }
  return { static: staticLibs, system };
}

// ---------------------------------------------------------------- package

async function packageArtifact(ffmpegBuild, startedAt) {
  await rm(out, { recursive: true, force: true });
  await mkdir(join(out, 'lib', 'pkgconfig'), { recursive: true });
  await cp(join(prefix, 'include'), join(out, 'include'), { recursive: true });
  for (const name of await readdir(join(prefix, 'lib'))) {
    if (name.toLowerCase().endsWith('.lib'))
      await copyFile(join(prefix, 'lib', name), join(out, 'lib', name));
  }
  // Relocatable .pc files: the artifact is unpacked anywhere.
  for (const file of await files(join(prefix, 'lib', 'pkgconfig'), (n) => n.endsWith('.pc'))) {
    const text = (await readFile(file, 'utf8')).replace(
      /^prefix=.*$/m,
      'prefix=${pcfiledir}/../..',
    );
    await writeFile(
      join(out, 'lib', 'pkgconfig', basename(file)),
      text.replaceAll(fwd(prefix), '${prefix}'),
    );
  }

  for (const source of lock.sources) {
    const dir = join(out, 'licenses', source.name);
    await mkdir(dir, { recursive: true });
    for (const file of source.licenseFiles) {
      await copyFile(join(sourceDir[source.name], file), join(dir, basename(file)));
    }
  }
  await copyFile(join(root, 'scripts', 'ffmpeg', 'RELINKING.md'), join(out, 'RELINKING.md'));
  await copyFile(
    join(root, 'scripts', 'ffmpeg', 'THIRD_PARTY_NOTICES.ffmpeg.md'),
    join(out, 'THIRD_PARTY_NOTICES.ffmpeg.md'),
  );

  const versionOf = (command, args, pattern) => {
    const result = spawnSync(command, args, { env: buildEnv, encoding: 'utf8' });
    return pattern.exec(`${result.stdout}${result.stderr}`)?.[1] ?? 'unknown';
  };
  const manifest = {
    schema: 1,
    name: artifactName,
    ffmpeg: ffmpegSource.version,
    revision: lock.revision,
    license: ffmpegSource.license,
    target: lock.target,
    crt: 'static (/MT, LIBCMT): link with -C target-feature=+crt-static',
    configure: configureLine,
    link: {
      ffmpeg: ['avfilter', 'avformat', 'avcodec', 'swresample', 'swscale', 'avutil'],
      ...(await linkLibraries(ffmpegBuild)),
    },
    toolchain: {
      msvc: toolsVersion,
      windows_sdk: (msvcEnv.WindowsSDKVersion ?? 'unknown').replace(/\\$/, ''),
      cmake: versionOf('cmake', ['--version'], /cmake version (\S+)/),
      nasm: lock.tools.find((t) => t.name === 'nasm').version,
      meson: lock.tools.find((t) => t.name === 'meson').version,
      ninja: lock.tools.find((t) => t.name === 'ninja').version,
      msys2: lock.tools.find((t) => t.name === 'msys2').version,
    },
    sources: lock.sources.map(({ name, version, url, sha256: digest, license }) =>
      overrides[name]
        ? { name, version, local: overrides[name], license }
        : { name, version, url, sha256: digest, license },
    ),
    built: new Date().toISOString(),
    build_seconds: Math.round((Date.now() - startedAt) / 1000),
    timings,
  };
  await writeFile(join(out, 'kinora-ffmpeg.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  return manifest;
}

async function writeDist() {
  await mkdir(join(dist, 'sources'), { recursive: true });
  const staging = join(work, 'dist', artifactName);
  await rm(dirname(staging), { recursive: true, force: true });
  await cp(out, staging, { recursive: true });
  const zip = join(dist, `${artifactName}.zip`);
  await rm(zip, { force: true });
  // bsdtar picks the zip format from the extension.
  run(systemTar, ['-a', '-cf', zip, '-C', dirname(staging), artifactName], { env: process.env });
  await writeFile(`${zip}.sha256`, `${await sha256(zip)}  ${basename(zip)}\n`);

  const sums = [];
  for (const source of lock.sources) {
    if (overrides[source.name]) continue; // the builder publishes their own source
    const file = archiveName(source);
    await copyFile(join(downloads, file), join(dist, 'sources', file));
    sums.push(`${source.sha256}  ${file}`);
  }
  await writeFile(join(dist, 'sources', 'SHA256SUMS'), `${sums.join('\n')}\n`);
  console.log(
    `dist: ${relative(process.cwd(), zip)} (${((await stat(zip)).size / 1048576).toFixed(1)} MB)`,
  );
}

// ---------------------------------------------------------------- main

const startedAt = Date.now();
console.log(`Kinora release FFmpeg ${ffmpegSource.version} r${lock.revision}`);
console.log(
  `work ${work}\ncache ${cache}\nout ${out}\nMSVC ${toolsVersion} (VS ${vsMajor}), ${jobs} jobs`,
);

await rm(work, { recursive: true, force: true });
await mkdir(work, { recursive: true });
await step('tools', setupTools);
await step('sources', fetchSources);
for (const name of [
  'zlib',
  'opus',
  'lame',
  'svt-av1',
  'libvpl',
  'dav1d',
  'libvpx',
  'nv-codec-headers',
  'amf-headers',
]) {
  await step(name, builders[name]);
}
await step('normalise dependencies', normaliseDependencies);
const ffmpegBuild = await step('ffmpeg', buildFfmpeg);
const manifest = await step('package', () => packageArtifact(ffmpegBuild, startedAt));
if (dist) await step('dist', writeDist);

console.log(`\nBuilt ${manifest.name} in ${manifest.build_seconds} s -> ${out}`);
if (!process.argv.includes('--skip-verify')) {
  const args = [join(root, 'scripts', 'ffmpeg', 'verify.mjs'), '--release', out];
  if (dist) args.push('--json', join(dist, 'verify-release.json'));
  const verify = spawnSync(process.execPath, args, { stdio: 'inherit' });
  process.exit(verify.status ?? 1);
}
