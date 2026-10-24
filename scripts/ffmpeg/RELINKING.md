# Rebuilding Kinora against a modified FFmpeg

Kinora's Windows release links FFmpeg and its codec libraries **statically** into `kinora.exe` (ADR-0005
Amendment 3). Two of those libraries are under the GNU Lesser General Public License:

| Library | Licence                                                                           |
| ------- | --------------------------------------------------------------------------------- |
| FFmpeg  | LGPL-2.1-or-later (built without `--enable-gpl`, `--enable-nonfree` or `--enable-version3`) |
| LAME    | LGPL-2.0-or-later (`libmp3lame`, the MP3 encoder)                                 |

The LGPL lets you modify these libraries and use your modified version with Kinora. Because they are linked
statically, you do that by rebuilding Kinora. Everything you need is public:

- Kinora's complete source code (MIT OR Apache-2.0): <https://github.com/Rvl08/kinora>.
- The build scripts for the static FFmpeg: [`scripts/ffmpeg/release/`](release/).
- The exact upstream source archives, pinned by SHA-256 in
  [`release-sources.json`](release-sources.json). Each Kinora release also publishes them next to the
  prebuilt libraries (`sources/` in the `ffmpeg-<version>-r<revision>-win64-lgpl-static` release), so
  they stay available even if an upstream site moves them.

## Requirements

- Windows 10 or 11, x64.
- Visual Studio 2022 or later (Community is enough) with "Desktop development with C++": the MSVC x64
  tools and a Windows SDK.
- CMake 3.21 or later, Node.js 24, Git, and Rust through [rustup](https://rustup.rs) (the version is
  pinned by `rust-toolchain.toml`).
- About 5 GB of free disk space and an internet connection for the first build. The build script fetches
  its other tools (MSYS2, NASM, Meson, Ninja) itself, checks their SHA-256, and never installs them
  system-wide.

## Steps

1. **Get the Kinora source of the release you use.**

   ```sh
   git clone https://github.com/Rvl08/kinora
   cd kinora
   git checkout <release tag>
   ```

2. **Prepare your modified library.** Unpack the pinned FFmpeg (or LAME) archive listed in
   `scripts/ffmpeg/release-sources.json`, apply your changes, and keep the tree somewhere with a short path
   without spaces, for example `C:\src\ffmpeg`.

3. **Build the static libraries with your source.** From a "x64 Native Tools Command Prompt for VS" (or any
   shell: the script finds Visual Studio itself):

   ```sh
   node scripts/ffmpeg/release/build.mjs --source ffmpeg=C:\src\ffmpeg
   ```

   Use `--source lame=C:\src\lame-3.100` for LAME; both options can be combined. The result is installed in
   `vendor/ffmpeg-static/` and checked by the same licence gate the official release uses
   (`node scripts/ffmpeg/verify.mjs --release`). The gate fails if your change enables a GPL or non-free
   component, because Kinora could then no longer be distributed under its licence; remove such options or
   distribute your build under the terms that apply to it.

4. **Build Kinora.** Release builds of Kinora read the static FFmpeg from `vendor/ffmpeg-static/`
   (`FFMPEG_DIR`) and compile with the static C runtime (`-C target-feature=+crt-static`). Follow the build
   instructions in the repository's `CONTRIBUTING.md` for the release you checked out. The resulting
   `kinora.exe` contains your modified library.

   To check the toolchain first, `spikes/ffmpeg-static-probe/` is a small program that links the same
   libraries the same way: `cd spikes/ffmpeg-static-probe && cargo run --release`.

## Notes

- `vendor/ffmpeg-static/kinora-ffmpeg.json` records the configure line, the toolchain versions and which
  sources were local (`"local": "<dir>"`) rather than pinned archives.
- The component set (codecs, formats, filters) comes from
  [`release-components.json`](release-components.json). If your modification adds a component, add it there
  too, or the gate reports it as unexpected.
- Kinora's own executables are signed. A rebuilt `kinora.exe` is not, so Windows Smart App Control may block
  it on machines where it is enabled.
