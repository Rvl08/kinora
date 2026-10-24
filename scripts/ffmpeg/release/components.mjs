// Turns scripts/ffmpeg/release-components.json into FFmpeg configure flags
// (build.mjs) and into the symbol names a static build must register
// (verify.mjs --release). Keeping both directions in one module means the
// build and the gate cannot drift apart.

import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

/**
 * One FFmpeg component group.
 * - option: configure switch (`--enable-<option>=name`).
 * - object: the libavcodec/libavformat/libavfilter object that holds the
 *   generated *_list.c, i.e. whose undefined references are exactly the
 *   registered components. null: no list; defined symbols are used instead.
 * - symbol: component name -> linker symbol.
 */
export const GROUPS = {
  decoders: {
    option: 'decoder',
    lib: 'avcodec',
    object: 'allcodecs.o',
    symbol: (n) => `ff_${n}_decoder`,
  },
  encoders: {
    option: 'encoder',
    lib: 'avcodec',
    object: 'allcodecs.o',
    symbol: (n) => `ff_${n}_encoder`,
  },
  parsers: {
    option: 'parser',
    lib: 'avcodec',
    object: 'parsers.o',
    symbol: (n) => `ff_${n}_parser`,
  },
  bsfs: {
    option: 'bsf',
    lib: 'avcodec',
    object: 'bitstream_filters.o',
    symbol: (n) => `ff_${n}_bsf`,
  },
  hwaccels: { option: 'hwaccel', lib: 'avcodec', object: null, symbol: (n) => `ff_${n}_hwaccel` },
  demuxers: {
    option: 'demuxer',
    lib: 'avformat',
    object: 'allformats.o',
    symbol: (n) => `ff_${n}_demuxer`,
  },
  muxers: {
    option: 'muxer',
    lib: 'avformat',
    object: 'allformats.o',
    symbol: (n) => `ff_${n}_muxer`,
  },
  protocols: {
    option: 'protocol',
    lib: 'avformat',
    object: 'protocols.o',
    symbol: (n) => `ff_${n}_protocol`,
  },
  filters: { option: 'filter', lib: 'avfilter', object: 'allfilters.o', symbol: null },
};

// Printed names (what `ffmpeg -demuxers` etc. show, and what the JSON uses)
// that configure and the linker spell differently. Hyphens become
// underscores generically (libvpx-vp9 -> libvpx_vp9).
const RENAMED = { demuxers: { mpeg: 'mpegps' } };

// Buffer sources and sinks are compiled into every libavfilter and cannot be
// named on the configure line (configure would warn "did not match anything").
const BUILTIN_FILTERS = {
  buffer: 'ff_vsrc_buffer',
  abuffer: 'ff_asrc_abuffer',
  buffersink: 'ff_vsink_buffer',
  abuffersink: 'ff_asink_abuffer',
};

/** Configure/linker spelling of a component listed in the JSON. */
export function configureName(group, name) {
  return RENAMED[group]?.[name] ?? name.replaceAll('-', '_');
}

/** Name of a filter from its FFFilter symbol (ff_vf_scale -> scale). */
export function filterName(symbol) {
  for (const [name, builtin] of Object.entries(BUILTIN_FILTERS)) {
    if (symbol === builtin) return name;
  }
  // Same pattern configure's find_filters_extern uses.
  return /^ff_[avfsinkrc]{2,5}_([A-Za-z0-9_]+)$/.exec(symbol)?.[1] ?? null;
}

export async function loadSpec(root) {
  const path = join(root, 'scripts', 'ffmpeg', 'release-components.json');
  return JSON.parse(await readFile(path, 'utf8'));
}

/** Component names per group, flattened and in configure spelling. */
export function wanted(spec) {
  const flat = (value) =>
    Array.isArray(value) ? value : Object.values(value ?? {}).flatMap((v) => flat(v));
  return Object.fromEntries(
    Object.keys(GROUPS).map((group) => [
      group,
      [...new Set(flat(spec[group]).map((name) => configureName(group, name)))].sort(),
    ]),
  );
}

/** Components the JSON accepts as pulled in by configure dependencies. */
export function implicit(spec) {
  return Object.fromEntries(
    Object.keys(GROUPS).map((group) => [
      group,
      (spec.implicit?.[group] ?? []).map((name) => configureName(group, name)),
    ]),
  );
}

/** External libraries and platform features, as configure feature names. */
export function features(spec) {
  return {
    external: [...spec.external_libraries.keep].sort(),
    platform: [...spec.platform].sort(),
  };
}

/**
 * The configure flags that select exactly the JSON's components on top of
 * --disable-everything --disable-autodetect.
 */
export function configureFlags(spec) {
  const flags = [];
  const lists = wanted(spec);
  for (const [group, { option }] of Object.entries(GROUPS)) {
    const names = lists[group].filter((n) => group !== 'filters' || !(n in BUILTIN_FILTERS));
    if (names.length) flags.push(`--enable-${option}=${names.join(',')}`);
  }
  const { external, platform } = features(spec);
  for (const feature of [...external, ...platform]) flags.push(`--enable-${feature}`);
  return flags;
}

/**
 * Maps a symbol back to its component name within a group, or null when the
 * symbol does not belong to the group.
 */
export function componentOf(group, symbol) {
  if (group === 'filters') return filterName(symbol);
  const probe = GROUPS[group].symbol('\u0000');
  const [prefix, suffix] = probe.split('\u0000');
  if (!symbol.startsWith(prefix) || !symbol.endsWith(suffix)) return null;
  return symbol.slice(prefix.length, symbol.length - suffix.length) || null;
}
