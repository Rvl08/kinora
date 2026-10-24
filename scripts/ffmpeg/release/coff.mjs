// Minimal reader for MSVC static libraries (COFF archives), so the release
// gate can inspect what a static FFmpeg build contains without running any
// of its code: Smart App Control blocks freshly built executables on the dev
// machine, and a library cannot be executed anyway.
//
// Format reference: Microsoft PE/COFF specification, sections "Archive
// (Library) File Format" and "COFF File Header". Only what the gate needs is
// implemented: member enumeration, the archive symbol index, per-object symbol
// tables and the .drectve linker directives.

const ARCHIVE_MAGIC = '!<arch>\n';
const HEADER_SIZE = 60;
const IMAGE_SYM_CLASS_EXTERNAL = 2;
// Anonymous object header: Sig1 = 0 (IMAGE_FILE_MACHINE_UNKNOWN), Sig2 = 0xFFFF.
const BIGOBJ_CLASS_ID = 'c7a1bad1eebaa94baf20faf66aa4dcb8'; // {D1BAA1C7-BAEE-4BA9-AF20-FAF66AA4DCB8}

/**
 * @typedef {{ name: string, offset: number, size: number }} Member
 * @typedef {{ defined: Set<string>, undefined: Set<string>, directives: string[], kind: string }} ObjectInfo
 */

/** Reads a COFF archive. Throws on anything that is not one. */
export function readArchive(buf) {
  if (buf.toString('latin1', 0, 8) !== ARCHIVE_MAGIC) throw new Error('not a COFF archive');
  /** @type {Member[]} */
  const members = [];
  const symbolIndex = new Set();
  let longNames = null;
  let linkerMembers = 0;
  let pos = 8;
  while (pos + HEADER_SIZE <= buf.length) {
    const rawName = buf.toString('latin1', pos, pos + 16).trimEnd();
    const size = Number.parseInt(buf.toString('latin1', pos + 48, pos + 58).trim(), 10);
    if (!Number.isFinite(size) || buf.toString('latin1', pos + 58, pos + 60) !== '`\n') {
      throw new Error(`corrupt archive member header at offset ${pos}`);
    }
    const offset = pos + HEADER_SIZE;
    if (rawName === '/') {
      // First linker member: big-endian symbol count, offsets, then names.
      if (linkerMembers++ === 0) {
        const count = buf.readUInt32BE(offset);
        let cursor = offset + 4 + 4 * count;
        for (let i = 0; i < count; i++) {
          const end = buf.indexOf(0, cursor);
          symbolIndex.add(buf.toString('latin1', cursor, end));
          cursor = end + 1;
        }
      }
    } else if (rawName === '//') {
      longNames = buf.subarray(offset, offset + size);
    } else {
      let name;
      if (/^\/\d+$/.test(rawName) && longNames) {
        const start = Number(rawName.slice(1));
        let end = start;
        while (end < longNames.length && longNames[end] !== 0 && longNames[end] !== 0x0a) end++;
        name = longNames.toString('latin1', start, end).replace(/\/$/, '');
      } else {
        name = rawName.replace(/\/$/, '');
      }
      members.push({ name: name.replaceAll('\\', '/'), offset, size });
    }
    pos = offset + size + (size % 2);
  }
  return { members, symbolIndex };
}

function symbolName(buf, recordOffset, stringTable) {
  if (buf.readUInt32LE(recordOffset) === 0) {
    const strOffset = buf.readUInt32LE(recordOffset + 4);
    const end = buf.indexOf(0, stringTable + strOffset);
    return buf.toString('latin1', stringTable + strOffset, end);
  }
  const raw = buf.subarray(recordOffset, recordOffset + 8);
  const end = raw.indexOf(0);
  return raw.toString('latin1', 0, end === -1 ? 8 : end);
}

/**
 * Parses one archive member as a COFF object.
 * kind: "coff", "bigobj", "import" (short import object) or "unknown"
 * (for example an LTCG /GL object, whose symbols are not readable).
 * @returns {ObjectInfo}
 */
export function readObject(archive, member) {
  const obj = archive.subarray(member.offset, member.offset + member.size);
  const info = { defined: new Set(), undefined: new Set(), directives: [], kind: 'coff' };
  let sections;
  let sectionTable;
  let symbolTable;
  let symbols;
  let recordSize;
  if (obj.readUInt16LE(0) === 0 && obj.readUInt16LE(2) === 0xffff) {
    const version = obj.readUInt16LE(4);
    const classId = obj.subarray(12, 28).toString('hex');
    if (version < 2 || classId !== BIGOBJ_CLASS_ID) {
      info.kind = version === 0 ? 'import' : 'unknown';
      return info;
    }
    info.kind = 'bigobj';
    sections = obj.readUInt32LE(44);
    symbolTable = obj.readUInt32LE(48);
    symbols = obj.readUInt32LE(52);
    sectionTable = 56;
    recordSize = 20;
  } else {
    sections = obj.readUInt16LE(2);
    symbolTable = obj.readUInt32LE(8);
    symbols = obj.readUInt32LE(12);
    sectionTable = 20 + obj.readUInt16LE(16);
    recordSize = 18;
  }

  for (let i = 0; i < sections; i++) {
    const header = sectionTable + 40 * i;
    const name = obj.toString('latin1', header, header + 8).replace(/\0+$/, '');
    if (name !== '.drectve') continue;
    const size = obj.readUInt32LE(header + 16);
    const data = obj.readUInt32LE(header + 20);
    // Directives are ASCII (or UTF-8 with a BOM), space separated, quotes allowed.
    const text = obj
      .toString('utf8', data, data + size)
      .replace(/^\uFEFF/, '')
      .replace(/\0/g, ' ');
    for (const match of text.matchAll(/(?:[^\s"]+|"[^"]*")+/g)) {
      info.directives.push(match[0].replaceAll('"', ''));
    }
  }

  const stringTable = symbolTable + symbols * recordSize;
  for (let i = 0; i < symbols; i++) {
    const record = symbolTable + i * recordSize;
    const section = recordSize === 20 ? obj.readInt32LE(record + 12) : obj.readInt16LE(record + 12);
    const storageClass = obj[record + recordSize - 2];
    const aux = obj[record + recordSize - 1];
    if (storageClass === IMAGE_SYM_CLASS_EXTERNAL) {
      const name = symbolName(obj, record, stringTable);
      const value = obj.readUInt32LE(record + 8);
      // Section 0 with value 0 is an undefined reference; with a value it is
      // a common symbol, which counts as a definition.
      if (section === 0 && value === 0) info.undefined.add(name);
      else info.defined.add(name);
    }
    i += aux;
  }
  return info;
}

/**
 * Returns every NUL-terminated printable ASCII string in `buf` that contains
 * `needle`. Used to read FFmpeg's embedded configuration and licence strings.
 */
export function findStrings(buf, needle) {
  const found = new Set();
  const pattern = Buffer.from(needle, 'latin1');
  let at = buf.indexOf(pattern);
  while (at !== -1) {
    let start = at;
    while (start > 0 && buf[start - 1] >= 0x20 && buf[start - 1] < 0x7f) start--;
    let end = at + pattern.length;
    while (end < buf.length && buf[end] >= 0x20 && buf[end] < 0x7f) end++;
    if (buf[end] === 0) found.add(buf.toString('latin1', start, end));
    at = buf.indexOf(pattern, end);
  }
  return [...found];
}
