// Minimal HEIF (HEIC) writer for the photo fixtures.
//
// FFmpeg can encode HEVC but has no HEIF muxer, and no freely licensed
// phone HEIC sample could be found (docs/spikes/P0-11-report.md), so
// generate.mjs encodes the tiles with FFmpeg and this module wraps them the
// way phones do: a 'grid' derived image item whose tiles are hidden 'hvc1'
// items sharing one hvcC (ISO/IEC 23008-12, ISO/IEC 14496-15).
//
// Input: an Annex B HEVC stream in which every picture is one intra-coded
// tile, in raster order. Output: ftyp + meta + mdat. No thumbnail, EXIF, ICC
// profile, depth map or gain map is written.

import { readFile, writeFile } from 'node:fs/promises';

/** Splits an Annex B byte stream into NAL units (without start codes). */
function splitNalUnits(data) {
  const starts = [];
  for (let i = 0; i + 2 < data.length; i++) {
    if (data[i] === 0 && data[i + 1] === 0 && data[i + 2] === 1) {
      starts.push(i + 3);
      i += 2;
    }
  }
  return starts.map((start, n) => {
    let end = n + 1 < starts.length ? starts[n + 1] - 3 : data.length;
    // A four-byte start code leaves a zero byte at the end of the previous unit.
    while (end > start && data[end - 1] === 0) end--;
    return data.subarray(start, end);
  });
}

const nalType = (nal) => (nal[0] >> 1) & 0x3f;

/** Removes emulation prevention bytes (00 00 03 -> 00 00). */
function toRbsp(nal) {
  const out = [];
  for (let i = 0; i < nal.length; i++) {
    if (i >= 2 && nal[i] === 3 && nal[i - 1] === 0 && nal[i - 2] === 0) continue;
    out.push(nal[i]);
  }
  return Buffer.from(out);
}

class BitReader {
  constructor(bytes) {
    this.bytes = bytes;
    this.pos = 0;
  }
  u(n) {
    let value = 0;
    for (let i = 0; i < n; i++) {
      const byte = this.bytes[this.pos >> 3];
      if (byte === undefined) throw new Error('SPS ended early');
      value = value * 2 + ((byte >> (7 - (this.pos & 7))) & 1);
      this.pos++;
    }
    return value;
  }
  ue() {
    let zeros = 0;
    while (this.u(1) === 0) zeros++;
    return 2 ** zeros - 1 + this.u(zeros);
  }
}

/** Reads the fields hvcC and ispe need from a sequence parameter set. */
function parseSps(nal) {
  const rbsp = toRbsp(nal);
  const r = new BitReader(rbsp.subarray(2)); // skip the NAL unit header
  r.u(4); // sps_video_parameter_set_id
  const maxSubLayersMinus1 = r.u(3);
  const temporalIdNesting = r.u(1);
  const ptlStart = r.pos >> 3; // the general profile_tier_level is byte aligned here
  const ptl = rbsp.subarray(2 + ptlStart, 2 + ptlStart + 12);
  r.pos += 96;
  const subLayers = [];
  for (let i = 0; i < maxSubLayersMinus1; i++) subLayers.push([r.u(1), r.u(1)]);
  if (maxSubLayersMinus1 > 0) for (let i = maxSubLayersMinus1; i < 8; i++) r.u(2);
  for (const [profilePresent, levelPresent] of subLayers) {
    if (profilePresent) r.pos += 88;
    if (levelPresent) r.pos += 8;
  }
  r.ue(); // sps_seq_parameter_set_id
  const chromaFormat = r.ue();
  if (chromaFormat === 3) r.u(1);
  let width = r.ue();
  let height = r.ue();
  if (r.u(1)) {
    const sub = chromaFormat === 1 || chromaFormat === 2 ? 2 : 1;
    const subV = chromaFormat === 1 ? 2 : 1;
    const [left, right, top, bottom] = [r.ue(), r.ue(), r.ue(), r.ue()];
    width -= sub * (left + right);
    height -= subV * (top + bottom);
  }
  return {
    ptl,
    maxSubLayersMinus1,
    temporalIdNesting,
    chromaFormat,
    width,
    height,
    bitDepthLuma: r.ue() + 8,
    bitDepthChroma: r.ue() + 8,
  };
}

const u8 = (v) => Buffer.from([v]);
const u16 = (v) => {
  const b = Buffer.alloc(2);
  b.writeUInt16BE(v);
  return b;
};
const u32 = (v) => {
  const b = Buffer.alloc(4);
  b.writeUInt32BE(v);
  return b;
};
const fourcc = (s) => Buffer.from(s, 'latin1');

function box(type, ...payload) {
  const body = Buffer.concat(payload);
  return Buffer.concat([u32(8 + body.length), fourcc(type), body]);
}

function fullBox(type, version, flags, ...payload) {
  return box(type, u8(version), u8(flags >> 16), u16(flags & 0xffff), ...payload);
}

function hvcC(sps, vps, spsNal, pps) {
  const arrays = [
    [32, vps],
    [33, spsNal],
    [34, pps],
  ].map(([type, nal]) => Buffer.concat([u8(0x80 | type), u16(1), u16(nal.length), nal]));
  return box(
    'hvcC',
    u8(1), // configurationVersion
    sps.ptl.subarray(0, 12), // profile space/tier/idc, compatibility, constraints, level
    u16(0xf000), // min_spatial_segmentation_idc = 0
    u8(0xfc), // parallelismType = 0
    u8(0xfc | sps.chromaFormat),
    u8(0xf8 | (sps.bitDepthLuma - 8)),
    u8(0xf8 | (sps.bitDepthChroma - 8)),
    u16(0), // avgFrameRate
    // constantFrameRate 0, numTemporalLayers, temporalIdNested, lengthSizeMinusOne 3
    u8(((sps.maxSubLayersMinus1 + 1) << 3) | (sps.temporalIdNesting << 2) | 3),
    u8(arrays.length),
    ...arrays,
  );
}

/**
 * Wraps the tiles of `hevcPath` into a HEIC grid image at `outPath`.
 *
 * @param {object} options
 * @param {string} options.hevcPath Annex B stream, one intra picture per tile.
 * @param {string} options.outPath HEIC file to write.
 * @param {number} options.columns Tiles per row.
 * @param {number} options.rows Tile rows.
 * @param {number} options.width Output width (the grid crops the right edge).
 * @param {number} options.height Output height (the grid crops the bottom edge).
 * @param {{primaries: number, transfer: number, matrix: number, fullRange: boolean}} options.nclx
 *   Colour description (ITU-T H.273 code points).
 */
export async function writeHeicGrid({ hevcPath, outPath, columns, rows, width, height, nclx }) {
  const nals = splitNalUnits(await readFile(hevcPath));
  const first = (type) => {
    const nal = nals.find((n) => nalType(n) === type);
    if (!nal) throw new Error(`no NAL unit of type ${type} in ${hevcPath}`);
    return nal;
  };
  const [vps, spsNal, pps] = [first(32), first(33), first(34)];
  const sps = parseSps(spsNal);

  // One picture per tile. A VCL unit with first_slice_segment_in_pic_flag set
  // starts a picture; SEI, AUD and repeated parameter sets are dropped.
  const tiles = [];
  for (const nal of nals) {
    if (nalType(nal) >= 32) continue;
    if (nal[2] & 0x80) tiles.push([]);
    if (tiles.length === 0) throw new Error('slice without a first slice segment');
    tiles.at(-1).push(nal);
  }
  if (tiles.length !== columns * rows) {
    throw new Error(`expected ${columns * rows} tiles, found ${tiles.length}`);
  }
  if (sps.width * columns < width || sps.height * rows < height) {
    throw new Error(`tiles of ${sps.width}x${sps.height} cannot cover ${width}x${height}`);
  }

  const tileData = tiles.map((units) =>
    Buffer.concat(units.flatMap((nal) => [u32(nal.length), nal])),
  );
  const gridData = Buffer.concat([
    u8(0), // version
    u8(0), // flags: 16-bit output size
    u8(rows - 1),
    u8(columns - 1),
    u16(width),
    u16(height),
  ]);
  const gridId = tiles.length + 1;
  const tileIds = tiles.map((_, i) => i + 1);

  const ftyp = box('ftyp', fourcc('heic'), u32(0), fourcc('mif1'), fourcc('heic'));
  const hdlr = fullBox('hdlr', 0, 0, u32(0), fourcc('pict'), u32(0), u32(0), u32(0), u8(0));
  const pitm = fullBox('pitm', 0, 0, u16(gridId));
  const infe = (id, type, flags) => fullBox('infe', 2, flags, u16(id), u16(0), fourcc(type), u8(0));
  const iinf = fullBox(
    'iinf',
    0,
    0,
    u16(gridId),
    ...tileIds.map((id) => infe(id, 'hvc1', 1)), // flag 1: hidden, shown only via the grid
    infe(gridId, 'grid', 0),
  );
  const iref = fullBox(
    'iref',
    0,
    0,
    box('dimg', u16(gridId), u16(tileIds.length), ...tileIds.map(u16)),
  );

  // Property indices are 1-based in ipma.
  const pixi = fullBox(
    'pixi',
    0,
    0,
    u8(3),
    u8(sps.bitDepthLuma),
    u8(sps.bitDepthChroma),
    u8(sps.bitDepthChroma),
  );
  const ipco = box(
    'ipco',
    hvcC(sps, vps, spsNal, pps), // 1
    fullBox('ispe', 0, 0, u32(sps.width), u32(sps.height)), // 2
    fullBox('ispe', 0, 0, u32(width), u32(height)), // 3
    box(
      'colr',
      fourcc('nclx'),
      u16(nclx.primaries),
      u16(nclx.transfer),
      u16(nclx.matrix),
      u8(nclx.fullRange ? 0x80 : 0),
    ), // 4
    pixi, // 5
  );
  const assoc = (id, ...props) => Buffer.concat([u16(id), u8(props.length), ...props.map(u8)]);
  const ipma = fullBox(
    'ipma',
    0,
    0,
    u32(gridId),
    ...tileIds.map((id) => assoc(id, 0x80 | 1, 2, 5)),
    assoc(gridId, 3, 4, 5),
  );
  const iprp = box('iprp', ipco, ipma);

  const items = [...tileData, gridData];
  const iloc = (base) => {
    let offset = base;
    const entries = items.map((data, i) => {
      const entry = Buffer.concat([u16(i + 1), u16(0), u16(1), u32(offset), u32(data.length)]);
      offset += data.length;
      return entry;
    });
    // offset_size 4, length_size 4, base_offset_size 0
    return fullBox('iloc', 0, 0, u8(0x44), u8(0), u16(items.length), ...entries);
  };
  // iloc has a fixed size, so the offsets can be filled in after measuring.
  const metaSize = fullBox('meta', 0, 0, hdlr, pitm, iinf, iref, iprp, iloc(0)).length;
  const meta = fullBox(
    'meta',
    0,
    0,
    hdlr,
    pitm,
    iinf,
    iref,
    iprp,
    iloc(ftyp.length + metaSize + 8),
  );
  await writeFile(outPath, Buffer.concat([ftyp, meta, box('mdat', ...items)]));
  return { tiles: tiles.length, tileWidth: sps.width, tileHeight: sps.height };
}
