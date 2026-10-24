// Release gate for Kinora's static FFmpeg (P0-05b), run as
// `node scripts/ffmpeg/verify.mjs --release [<dir>] [--json <path>]`.
//
// Nothing is executed: the checks read the static libraries (COFF archives)
// directly, so the gate runs on any machine, Smart App Control included.
// Fails on:
// - --enable-gpl, --enable-nonfree, a banned or ship-blocking library (in the
//   embedded configuration or as linked symbols, e.g. libzvbi, libopenh264);
// - a licence string other than "LGPL version 2.1/3 or later";
// - an FFmpeg version other than release-sources.json's;
// - a component set different from release-components.json (missing, or an
//   extra one that the JSON does not list as implicit);
// - an object that asks for the DLL CRT (/MD) or uses link-time code
//   generation (/GL), and any DLL or EXE in the artifact.
// Reports library sizes.

import { existsSync } from 'node:fs';
import { readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { basename, join } from 'node:path';

import { BANNED, SHIP_BLOCKERS } from '../policy.mjs';
import { findStrings, readArchive, readObject } from './coff.mjs';
import { GROUPS, componentOf, features, implicit, loadSpec, wanted } from './components.mjs';

const FFMPEG_LIBS = ['avutil', 'avcodec', 'avformat', 'avfilter', 'swscale', 'swresample'];

// Symbols that prove a forbidden library was linked in, whatever the
// configuration string says.
const FORBIDDEN_SYMBOLS = [
  [/^Wels(Create|Destroy)(SVCEncoder|Decoder)$/, 'OpenH264 (Cisco DLL at run time only)'],
  [/^vbi_/, 'libzvbi (GPL-2.0-or-later)'],
  [/^x264_/, 'x264 (GPL)'],
  [/^x265_/, 'x265 (GPL)'],
  [/^xvid_/, 'Xvid (GPL)'],
  [/^aacEnc(Open|Encode)$/, 'fdk-aac (non-free)'],
  [/^vs(MotionDetect|Transform)/, 'vid.stab (GPL)'],
  [/^ff_lib(openh264|zvbi|x264|x265|xvid|fdk_aac)_/, 'banned FFmpeg wrapper'],
];

// /DEFAULTLIB values of the static CRT (allowed) and of the DLL or debug CRT (not).
const STATIC_CRT = new Set([
  'libcmt',
  'libucrt',
  'libvcruntime',
  'libcpmt',
  'oldnames',
  'libconcrt',
]);
const WRONG_CRT = new Set([
  'msvcrt',
  'msvcrtd',
  'msvcprt',
  'msvcprtd',
  'ucrt',
  'ucrtd',
  'vcruntime',
  'vcruntimed',
  'libcmtd',
  'libucrtd',
  'libvcruntimed',
  'libcpmtd',
  'concrt',
]);

const mb = (bytes) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;
const difference = (a, b) => [...a].filter((x) => !b.has(x)).sort();

export async function verifyRelease({ root, dir, jsonPath }) {
  const failures = [];
  const notes = [];
  const spec = await loadSpec(root);
  const sources = JSON.parse(
    await readFile(join(root, 'scripts', 'ffmpeg', 'release-sources.json'), 'utf8'),
  );
  const ffmpegVersion = sources.sources.find((s) => s.name === 'ffmpeg').version;

  if (!existsSync(join(dir, 'lib', 'avcodec.lib'))) {
    console.error(`No static FFmpeg in ${dir}. Run node scripts/ffmpeg/release/build.mjs first.`);
    return 2;
  }

  // ------------------------------------------------------------ layout
  const all = await readdir(dir, { recursive: true, withFileTypes: true });
  for (const entry of all) {
    if (entry.isFile() && /\.(dll|exe)$/i.test(entry.name)) {
      failures.push(`binary in a static artifact: ${join(entry.parentPath, entry.name)}`);
    }
  }
  const libFiles = (await readdir(join(dir, 'lib'))).filter((n) =>
    n.toLowerCase().endsWith('.lib'),
  );
  for (const name of FFMPEG_LIBS) {
    if (!libFiles.includes(`${name}.lib`)) failures.push(`missing lib/${name}.lib`);
  }
  let manifest = null;
  try {
    manifest = JSON.parse(await readFile(join(dir, 'kinora-ffmpeg.json'), 'utf8'));
  } catch {
    failures.push('missing or unreadable kinora-ffmpeg.json');
  }

  // ------------------------------------------------------------ libraries
  const archives = {};
  const sizes = {};
  for (const file of libFiles) {
    const name = file.slice(0, -4);
    const buf = await readFile(join(dir, 'lib', file));
    sizes[name] = buf.length;
    try {
      archives[name] = { buf, ...readArchive(buf) };
    } catch (error) {
      failures.push(`lib/${file}: ${error.message}`);
    }
  }

  // ------------------------------------------------------------ version
  const ffversion = await readFile(join(dir, 'include', 'libavutil', 'ffversion.h'), 'utf8').catch(
    () => '',
  );
  const version = /#define FFMPEG_VERSION "([^"]+)"/.exec(ffversion)?.[1] ?? 'unknown';
  if (version !== ffmpegVersion && !version.startsWith(`${ffmpegVersion}-`)) {
    failures.push(
      `version mismatch: release-sources.json pins ${ffmpegVersion}, headers say ${version}`,
    );
  }

  // ------------------------------------------------------------ configuration
  const configurations = new Set();
  for (const name of FFMPEG_LIBS) {
    if (!archives[name]) continue;
    for (const text of findStrings(archives[name].buf, '--disable-everything')) {
      configurations.add(text);
    }
  }
  if (configurations.size !== 1) {
    failures.push(`expected one embedded configure line, found ${configurations.size}`);
  }
  const configuration = [...configurations][0] ?? '';
  const flags = configuration.split(/\s+/).filter((t) => t.startsWith('--'));
  for (const flag of ['--enable-gpl', '--enable-nonfree']) {
    if (flags.includes(flag)) failures.push(`build configured with ${flag}`);
  }
  if (flags.includes('--enable-version3')) notes.push('--enable-version3: licence is LGPL v3');
  if (flags.includes('--enable-shared') || !flags.includes('--enable-static')) {
    failures.push('not a static-only build (--enable-static, no --enable-shared)');
  }
  const enabled = new Set(
    flags.filter((f) => /^--enable-[a-z0-9_]+$/.test(f)).map((f) => f.slice('--enable-'.length)),
  );
  for (const lib of [...BANNED, ...Object.keys(SHIP_BLOCKERS)]) {
    if (enabled.has(lib.replaceAll('-', '_')) || enabled.has(lib)) {
      failures.push(
        `forbidden library enabled: ${lib}${SHIP_BLOCKERS[lib] ? ` (${SHIP_BLOCKERS[lib]})` : ''}`,
      );
    }
  }
  const { external, platform } = features(spec);
  const externalEnabled = [...enabled].filter(
    (f) => f.startsWith('lib') || ['amf', 'ffnvcodec', 'zlib'].includes(f),
  );
  for (const lib of difference(new Set(external), new Set(externalEnabled))) {
    failures.push(`external library not enabled: ${lib}`);
  }
  for (const lib of difference(new Set(externalEnabled), new Set(external))) {
    failures.push(`external library not in release-components.json: ${lib}`);
  }
  for (const feature of platform) {
    if (!enabled.has(feature)) failures.push(`platform feature not enabled: ${feature}`);
  }

  // ------------------------------------------------------------ licence
  const licences = {};
  for (const name of FFMPEG_LIBS) {
    if (!archives[name]) continue;
    const text = findStrings(archives[name].buf, `lib${name} license: `)[0] ?? '';
    licences[name] = text.slice(text.indexOf('license: ') + 'license: '.length) || 'unknown';
    if (!/^LGPL version (2\.1|3) or later$/.test(licences[name])) {
      failures.push(`lib${name} licence is "${licences[name]}", not the LGPL`);
    }
  }
  const licence = [...new Set(Object.values(licences))].join(', ');

  // ------------------------------------------------------------ components
  const want = wanted(spec);
  const allowed = implicit(spec);
  const components = {};
  for (const [group, { lib, object }] of Object.entries(GROUPS)) {
    const archive = archives[lib];
    if (!archive) continue;
    let symbols;
    if (object) {
      const member = archive.members.find((m) => basename(m.name) === object);
      if (!member) {
        failures.push(`${lib}.lib has no ${object}; cannot read the ${group} list`);
        continue;
      }
      symbols = readObject(archive.buf, member).undefined;
    } else {
      symbols = archive.symbolIndex;
    }
    const have = new Set([...symbols].map((s) => componentOf(group, s)).filter(Boolean));
    const expected = new Set(want[group]);
    const missing = difference(expected, have);
    const extra = difference(have, expected);
    const unexpected = extra.filter((n) => !allowed[group].includes(n));
    const unusedImplicit = allowed[group].filter((n) => !have.has(n));
    if (missing.length) failures.push(`${group} missing: ${missing.join(', ')}`);
    if (unexpected.length) {
      failures.push(
        `${group} not in release-components.json: ${unexpected.join(', ')} (list them under "implicit" if configure selects them)`,
      );
    }
    if (unusedImplicit.length)
      notes.push(`${group} listed as implicit but absent: ${unusedImplicit.join(', ')}`);
    components[group] = {
      expected: expected.size,
      built: have.size,
      implicit: extra.length - unexpected.length,
    };
  }
  const hwdevices = {};
  if (archives.avutil) {
    const types = [...archives.avutil.symbolIndex]
      .map((s) => /^ff_hwcontext_type_(\w+)$/.exec(s)?.[1])
      .filter(Boolean);
    for (const device of spec.hwdevices) {
      hwdevices[device] = types.includes(device);
      if (!hwdevices[device]) failures.push(`hardware device type missing: ${device}`);
    }
    const others = types.filter((t) => !spec.hwdevices.includes(t));
    if (others.length)
      failures.push(`hardware device types not in release-components.json: ${others.join(', ')}`);
  }

  // ------------------------------------------------------------ symbols and CRT
  const crt = {};
  for (const [name, archive] of Object.entries(archives)) {
    for (const symbol of archive.symbolIndex) {
      for (const [pattern, what] of FORBIDDEN_SYMBOLS) {
        if (pattern.test(symbol)) failures.push(`${name}.lib defines ${symbol}: ${what}`);
      }
    }
    const usage = { objects: 0, static: 0, none: 0 };
    for (const member of archive.members) {
      const object = readObject(archive.buf, member);
      if (object.kind === 'import') {
        failures.push(`${name}.lib is (or contains) an import library: link it statically`);
        break;
      }
      if (object.kind === 'unknown') {
        failures.push(`${name}.lib: ${member.name} is not a plain COFF object (/GL?)`);
        continue;
      }
      usage.objects++;
      const defaults = object.directives
        .filter((d) => /^[-/]defaultlib:/i.test(d))
        .map((d) =>
          d
            .slice(d.indexOf(':') + 1)
            .toLowerCase()
            .replace(/\.lib$/, ''),
        );
      const wrong = defaults.filter((d) => WRONG_CRT.has(d));
      if (wrong.length) {
        failures.push(
          `${name}.lib: ${member.name} uses the ${wrong.join('/')} CRT (/MD or debug), not /MT`,
        );
      }
      if (defaults.some((d) => STATIC_CRT.has(d))) usage.static++;
      else usage.none++;
    }
    crt[name] = usage;
  }

  // ------------------------------------------------------------ manifest
  if (manifest) {
    for (const name of [...(manifest.link?.ffmpeg ?? []), ...(manifest.link?.static ?? [])]) {
      if (!libFiles.includes(`${name}.lib`))
        failures.push(`manifest links ${name}.lib, which is missing`);
    }
    if (manifest.ffmpeg !== ffmpegVersion) failures.push(`manifest says FFmpeg ${manifest.ffmpeg}`);
  }

  // ------------------------------------------------------------ sizes
  const ffmpegBytes = FFMPEG_LIBS.reduce((sum, n) => sum + (sizes[n] ?? 0), 0);
  const totalBytes = Object.values(sizes).reduce((sum, n) => sum + n, 0);
  let headerBytes = 0;
  for (const entry of all) {
    if (entry.isFile() && entry.parentPath.includes(`${join(dir, 'include')}`)) {
      headerBytes += (await stat(join(entry.parentPath, entry.name))).size;
    }
  }

  const report = {
    ok: failures.length === 0,
    mode: 'release',
    dir,
    version,
    licence,
    configuration,
    components,
    hwdevices,
    crt,
    sizes: { libraries: sizes, ffmpeg: ffmpegBytes, total: totalBytes, headers: headerBytes },
    failures,
    notes,
  };
  if (jsonPath) await writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`);

  console.log(`FFmpeg ${version}, ${licence}`);
  for (const [group, c] of Object.entries(components)) {
    console.log(`${group}: ${c.built} built (${c.expected} listed, ${c.implicit} implicit)`);
  }
  console.log(
    `hardware devices: ${Object.keys(hwdevices)
      .filter((d) => hwdevices[d])
      .join(', ')}`,
  );
  const crtSummary = Object.entries(crt)
    .map(([n, u]) => `${n} ${u.static}/${u.objects}`)
    .join(', ');
  console.log(`static CRT (objects asking for LIBCMT/total): ${crtSummary}`);
  console.log(
    `sizes: FFmpeg ${mb(ffmpegBytes)}, all libraries ${mb(totalBytes)}, headers ${mb(headerBytes)}`,
  );
  for (const [name, bytes] of Object.entries(sizes).sort((a, b) => b[1] - a[1])) {
    console.log(`  ${name}.lib ${mb(bytes)}`);
  }
  for (const note of notes) console.log(`NOTE ${note}`);
  for (const failure of failures) console.error(`FAIL ${failure}`);
  console.log(`FFmpeg licence check (release): ${report.ok ? 'OK' : 'FAILED'}`);
  return report.ok ? 0 : 1;
}
