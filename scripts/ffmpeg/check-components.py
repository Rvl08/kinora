"""Checks every name in release-components.json against what an FFmpeg build
actually provides, so the P0-05b configure line never names a component that
does not exist.

Usage: python scripts/ffmpeg/check-components.py [path/to/ffmpeg.exe]
"""

import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
FFMPEG = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "vendor/ffmpeg-lgpl/bin/ffmpeg.exe"
SPEC = json.loads((ROOT / "scripts/ffmpeg/release-components.json").read_text(encoding="utf-8"))


def listing(kind: str) -> set[str]:
    out = subprocess.run([str(FFMPEG), "-hide_banner", f"-{kind}"], capture_output=True, text=True).stdout
    names: set[str] = set()
    for line in out.splitlines():
        line = line.strip()
        if kind in ("bsfs", "protocols"):
            if line and not line.endswith(":") and " " not in line:
                names.add(line)
            continue
        cols = line.split()
        # Flag column then name: " V....D h264 ..." / " D  mov,mp4,... " / " .. scale  V->V ..."
        if len(cols) >= 2 and re.fullmatch(r"[A-Z.|]{1,8}", cols[0]):
            names.update(cols[1].split(","))
    return names


def main() -> int:
    groups = {
        "decoders": [n for v in SPEC["decoders"].values() for n in v],
        "encoders": [n for v in SPEC["encoders"].values() for n in v],
        "demuxers": SPEC["demuxers"],
        "muxers": SPEC["muxers"],
        "bsfs": SPEC["bsfs"],
        "protocols": SPEC["protocols"],
        "filters": SPEC["filters"],
    }
    missing = 0
    for kind, wanted in groups.items():
        have = listing(kind)
        absent = [n for n in wanted if n not in have]
        missing += len(absent)
        status = "ok" if not absent else f"MISSING {absent}"
        print(f"{kind:<10} {len(wanted):>3} named, {len(have):>3} in build: {status}")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
