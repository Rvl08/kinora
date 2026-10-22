# Test media fixtures

A reproducible, freely licensed set of media files for codec, import, preview, export and AI tests
(task P0-11, [`docs/04-phase0-plan.md`](../../docs/04-phase0-plan.md) §6). The files are kept out
of git in `tests/media/fixtures/`; this folder holds the scripts that download or generate them and
the hashes that pin them. Report and gaps: [`docs/spikes/P0-11-report.md`](../../docs/spikes/P0-11-report.md).
Credits: [`ATTRIBUTION.md`](ATTRIBUTION.md).

## Usage

Needs Node 24 and the dev FFmpeg (`pnpm run ffmpeg:fetch`), Windows (bsdtar and the Consolas font
are taken from the system).

```sh
node scripts/fixtures/fetch.mjs      # downloads (~330 MB transferred), keeps ~50 MB
node scripts/fixtures/generate.mjs   # synthetic clips, ~255 MB (~35 s with an NVIDIA GPU)
node scripts/fixtures/verify.mjs     # table of every fixture; exit code 1 on a failure
```

Options (all scripts): `--ids a,b` limits the run to some fixtures. `fetch` and `generate`:
`--force` redoes fixtures that already match, `--pin` writes new hashes into `manifest.json`
(maintainers, see below). `fetch --keep-sources` keeps the downloaded originals of derived fixtures.
`generate --allow-skip` exits 0 when a fixture has no usable encoder. `verify --allow-missing`
reports absent fixtures without failing.

All three are idempotent: a fixture whose files already match is left alone.

## Fixtures

| Id | File | Kind | Covers |
|---|---|---|---|
| `video-h264-1080p30-vfr` | `video/h264-1080p30-vfr.mp4` | generate | H.264 High, 1080p, VFR like a phone |
| `video-hevc-2160p60-hdr10` | `video/hevc-2160p60-main10-hdr10.mp4` | generate (GPU) | HEVC Main 10, 4K60, BT.2020 PQ, HDR10 metadata |
| `video-prores422-2160p` | `video/prores422-2160p23.976.mov` | generate | ProRes 422, 4K, 23.976 fps, 24-bit PCM |
| `video-av1-1080p30` | `video/av1-1080p30.mp4` | generate | AV1 Main, 1080p30, AAC |
| `video-vp9-alpha-1080p30` | `video/vp9-alpha-1080p30.webm` | generate | VP9 with alpha in WebM |
| `video-screen-1440p30-lossless` | `video/screen-1440p30-ffv1.mkv` | generate | Screen recording with sharp text, lossless RGB |
| `photo-jpeg-phone` | `photo/jpeg-iphone15pro-p3-gps.jpg` | download (CC0) | iPhone JPEG, Display P3, EXIF with GPS |
| `photo-jpeg-phone-rotated` | `photo/jpeg-iphone11-exif-rotate90.jpg` | download (CC0) | iPhone JPEG with EXIF Orientation 6 |
| `photo-heic-grid` | `photo/heic-grid-4032x3024.heic` | generate | HEIC with an iPhone-style 8x6 tile grid |
| `audio-id-podcast` | `audio/id-podcast-glam-6min.opus` | download (CC BY) | Spontaneous Indonesian talk, filler words |
| `audio-id-interview` | `audio/id-interview-2speakers.opus` | download (CC BY) | Indonesian two-speaker interview |
| `audio-song-vocals` | `audio/song-stickybee-vocals.ogg` | download (CC BY) | Song with vocals |
| `audio-song-instrumental` | `audio/song-stickybee-instrumental.ogg` | download (CC BY) | Same song without vocals |
| `speech-id-fleurs-transcripts` | `audio/fleurs-id-dev/dev.tsv` | download (CC BY) | FLEURS id_id dev transcripts |
| `speech-id-fleurs-clips` | `audio/fleurs-id-dev/*.wav` (25) | download (CC BY) | Indonesian read speech (Spike S3 WER set) |

`manifest.json` says for each fixture what it is for (`purpose`), its licence and attribution, the
pinned source or the exact FFmpeg recipe, the expected properties and the file hashes.

## Manifest format

```jsonc
{
  "id": "video-av1-1080p30",
  "purpose": "…",
  "kind": "download" | "generate",
  "licence": "CC0-1.0",                      // SPDX-like; "MIT OR Apache-2.0" for our own output
  "attribution": "…",                        // required for CC BY sources
  "needs": "…",                              // hardware a recipe depends on, if any
  "source": {                                // downloads only
    "url": "https://…",                      // immutable where possible (HF commit, Commons original)
    "page": "https://…",                     // licence/description page
    "range": [0, 16777215],                  // optional: only these bytes are downloaded and pinned
    "bytes": 16777216, "sha256": "…",        // of the downloaded bytes (the range, if any)
    "sha1": "…", "fileBytes": 69312362       // Commons' SHA-1 / size of the whole original, for reference
  },
  "transform": { "ffmpeg": ["-i", "{src}", …, "{out}"] }   // or { "untar": true } with files[].member
  "recipe": {                                // generated fixtures
    "encoders": { "libsvtav1": ["-c:v", …], "libaom-av1": [ … ] },   // in order of preference
    "steps": [{ "ffmpeg": [ …, "{encoder}", …, "{out}" ] }, { "heif": { … } }]
  },
  "files": [{ "path": "video/av1-1080p30.mp4", "bytes": 0, "sha256": "…" }],
  "pin": { "ffmpeg": "n8.1.3-18-g46d899d7a0", "encoder": "libsvtav1" },
  "expect": { "format": "…", "duration": [9.9, 10.1], "video": { … }, "audio": { … } }
}
```

Recipe placeholders: `{out}`, `{tmp}` (scratch directory), `{encoder}` (the chosen encoder's
arguments), `{font}` and `{file:path}` (paths escaped for filter graphs), `{fixture:id}` (another
fixture, which must exist first: run `fetch` before `generate`).

## How fixtures are verified

FFmpeg output is not byte-identical across FFmpeg builds, and hardware encoders differ across GPUs
and drivers. So every file gets two checks:

1. **SHA-256.**
   - Downloads and archive members are exact bytes. Any difference fails, and `fetch` refuses
     a download whose size or hash differs from the manifest.
   - Files made by the local FFmpeg (generated clips, audio excerpts) are pinned for one FFmpeg build
     and encoder (`pin`). `fetch` and `generate` record in `tests/media/fixtures/.state.json` which build
     and encoder made each local file. A difference from the pin then fails only when this machine used
     the pinned build and the same *software* encoder (a reproducibility bug), or when the file changed
     after it was made. Otherwise it is reported as `differs`, with the reason.
2. **Properties** with ffprobe (`expect`), always enforced:
   - container, codec, profile, size and pixel format (bit depth);
   - average frame rate, and VFR (more than one distinct frame duration);
   - colour primaries, transfer, matrix and range;
   - HDR10 mastering display and MaxCLL/MaxFALL, alpha (`alpha_mode`);
   - EXIF rotation and ICC profile of the first frame, the HEIF tile grid, duration.

All generated fixtures, NVENC ones included, gave identical bytes on repeated runs with the pinned
build on the reference machine (RTX 5070 Ti). Only the software encoders are promised to do so
elsewhere.

## Encoders and hardware

The dev FFmpeg is an LGPL build without libx264/libx265. `generate.mjs` probes each candidate
encoder with a short test encode and takes the first that works:

| Fixture | Preferred | Fallback without an NVIDIA GPU |
|---|---|---|
| H.264 VFR | `h264_nvenc` (High, B-frames) | `libopenh264` (High, CABAC, no B-frames) |
| HEVC 10-bit HDR10 | `hevc_nvenc` | none: skipped (no 10-bit software HEVC encoder in this build) |
| AV1 | `libsvtav1` | `libaom-av1` |
| VP9 alpha, ProRes, FFV1 | `libvpx-vp9`, `prores_ks`, `ffv1` | (software) |
| HEIC tiles | `libkvazaar` | `hevc_nvenc` |

On a runner without NVIDIA hardware use `generate.mjs --allow-skip` and `verify.mjs --allow-missing`.

## Updating the set

1. Add or change an entry in `manifest.json`. For a new download, check the licence on the source
   page (for Wikimedia Commons: `action=query&prop=imageinfo&iiprop=extmetadata|sha1`), then pin its
   size and SHA-256 by hand. `fetch` never trusts an unpinned URL. Allowed licences: CC0, public
   domain, CC BY (add the credit to `ATTRIBUTION.md`); no NC, ND or GPL media.
2. Run `fetch.mjs --pin --ids <id>` or `generate.mjs --pin --force --ids <id>`. This writes the hash of
   files made by FFmpeg together with `pin.ffmpeg`/`pin.encoder`.
3. Run `verify.mjs`, review the manifest diff and run `pnpm run format`.

When the pinned FFmpeg (`scripts/ffmpeg/lock.json`) changes, regenerate with `--pin --force` so the
pins follow the new build.
