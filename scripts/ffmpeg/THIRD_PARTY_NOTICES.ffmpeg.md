# Third-party notices: FFmpeg and codec libraries

This fragment is part of Kinora's `THIRD_PARTY_NOTICES`. It covers the multimedia libraries that are linked
statically into `kinora.exe` on Windows. The exact versions and SHA-256 digests are pinned in
[`scripts/ffmpeg/release-sources.json`](release-sources.json); the full licence texts ship in the
`licenses/<name>/` folder of the static FFmpeg artifact and in the installer.

**Source code.** The complete corresponding source of every library below is published with each Kinora
release (the `sources/` folder of the `ffmpeg-<version>-r<revision>-win64-lgpl-static` GitHub release),
together with the scripts that build them ([`scripts/ffmpeg/release/`](release/)). How to rebuild Kinora with
a modified FFmpeg or LAME, as the LGPL allows: [`RELINKING.md`](RELINKING.md).

| Library                 | Version   | Licence                                                    | Licence text in the artifact                           | Source                                                                                      |
| ----------------------- | --------- | ---------------------------------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------- |
| FFmpeg                  | 8.1.3     | LGPL-2.1-or-later                                          | `licenses/ffmpeg/COPYING.LGPLv2.1`, `LICENSE.md`       | <https://ffmpeg.org/releases/ffmpeg-8.1.3.tar.xz>                                           |
| LAME (libmp3lame)       | 3.100     | LGPL-2.0-or-later                                          | `licenses/lame/COPYING`, `LICENSE`                     | <https://downloads.sourceforge.net/project/lame/lame/3.100/lame-3.100.tar.gz>               |
| dav1d                   | 1.5.4     | BSD-2-Clause                                               | `licenses/dav1d/COPYING`                               | <https://downloads.videolan.org/pub/videolan/dav1d/1.5.4/dav1d-1.5.4.tar.xz>                |
| SVT-AV1                 | 4.0.1     | BSD-3-Clause-Clear, plus the AOMedia Patent License 1.0    | `licenses/svt-av1/LICENSE.md`, `LICENSE-BSD2.md`, `PATENTS.md` | <https://github.com/AOMediaCodec/SVT-AV1/archive/refs/tags/v4.0.1.tar.gz>           |
| libvpx                  | 1.17.0    | BSD-3-Clause, plus Google's additional patent grant        | `licenses/libvpx/LICENSE`, `PATENTS`, `AUTHORS`        | <https://github.com/webmproject/libvpx/archive/refs/tags/v1.17.0.tar.gz>                    |
| Opus                    | 1.6.1     | BSD-3-Clause                                               | `licenses/opus/COPYING`, `AUTHORS`                     | <https://downloads.xiph.org/releases/opus/opus-1.6.1.tar.gz>                                |
| Intel VPL (libvpl)      | 2.17.0    | MIT                                                        | `licenses/libvpl/LICENSE`                              | <https://github.com/intel/libvpl/archive/refs/tags/v2.17.0.tar.gz>                          |
| nv-codec-headers        | 12.2.72.0 | MIT (header notice)                                        | `licenses/nv-codec-headers/nvEncodeAPI.h`, `README`    | <https://github.com/FFmpeg/nv-codec-headers/releases/download/n12.2.72.0/nv-codec-headers-12.2.72.0.tar.gz> |
| AMD AMF headers         | 1.5.3     | MIT (header notice)                                        | `licenses/amf-headers/Version.h`                       | <https://github.com/GPUOpen-LibrariesAndSDKs/AMF/releases/download/v1.5.3/AMF-headers-v1.5.3.tar.gz> |
| zlib                    | 1.3.2     | Zlib                                                       | `licenses/zlib/LICENSE`                                | <https://github.com/madler/zlib/releases/download/v1.3.2/zlib-1.3.2.tar.xz>                 |

## Notes

- **FFmpeg** is configured without `--enable-gpl`, `--enable-nonfree` and `--enable-version3`, so it is
  licensed under the GNU Lesser General Public License, version 2.1 or (at your option) any later version.
  The configure line is embedded in the libraries (`avcodec_configuration()`) and recorded in
  `kinora-ffmpeg.json`.
- **Not included:** x264, x265, fdk-aac, OpenH264, libzvbi and any other GPL or non-free component. Kinora's
  software H.264 encoder is Cisco's OpenH264 binary, downloaded at run time under Cisco's own licence; it is
  not part of `kinora.exe`.
- **Hardware encoders:** NVIDIA NVENC/NVDEC, Intel Quick Sync (through Intel VPL) and AMD AMF are reached
  through the vendors' drivers, which Kinora loads at run time and does not distribute. The NVIDIA and AMD
  headers above only describe those interfaces.
- **Patents:** the SVT-AV1 and libvpx licences include patent grants (`PATENTS.md`, `PATENTS`). Neither
  Kinora nor these licences grant rights to third-party patents on codec standards (for example H.264, HEVC
  or AAC); see AMD's notice in `Version.h`.
