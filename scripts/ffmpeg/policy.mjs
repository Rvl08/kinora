// Licence policy shared by the development check (verify.mjs) and the release
// gate (verify.mjs --release). Source: ADR-0005 and docs/05-license-compliance.md.

// GPL or non-free external libraries. FFmpeg's configure already refuses most
// of them without --enable-gpl/--enable-nonfree; this list is defence in depth.
export const BANNED = [
  'libx264',
  'libx265',
  'libxavs',
  'libxavs2',
  'libxvid',
  'libvidstab',
  'librubberband',
  'libfdk-aac',
  'libdavs2',
  'libcdio',
  'frei0r',
];

// Allowed for development, not cleared for distribution.
export const SHIP_BLOCKERS = {
  libzvbi:
    'zvbi is GPL-2.0-or-later unless a file says otherwise (upstream COPYING.md); FFmpeg 8.1 configure does not flag it as GPL, but Kinora treats it as GPL until proven otherwise. Teletext is not needed: drop it from the shipping build.',
  libopenh264:
    "Cisco's patent licence covers only Cisco-built binaries. A statically built OpenH264 is not covered: load Cisco's DLL at runtime or drop it (Spike S2).",
};
