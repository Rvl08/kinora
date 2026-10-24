//! Links what the static FFmpeg needs besides its own six libraries.
//!
//! ffmpeg-sys-next links avcodec.lib & co. from FFMPEG_DIR/lib. The external
//! codec libraries (dav1d, SVT-AV1, ...) and the Windows system libraries come
//! from the artifact's manifest, kinora-ffmpeg.json, written by
//! scripts/ffmpeg/release/build.mjs, so this list never drifts from the build.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    let dir =
        PathBuf::from(env::var("FFMPEG_DIR").expect("FFMPEG_DIR (set by .cargo/config.toml)"));
    let manifest_path = dir.join("kinora-ffmpeg.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let text = fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}. Build it with `node scripts/ffmpeg/release/build.mjs`.",
            manifest_path.display()
        )
    });
    let manifest: serde_json::Value = serde_json::from_str(&text).expect("kinora-ffmpeg.json");

    // Same CRT as the libraries, or the link mixes LIBCMT and MSVCRT.
    let features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    assert!(
        features.split(',').any(|f| f == "crt-static"),
        "the release FFmpeg is built with /MT: compile with -C target-feature=+crt-static"
    );

    println!(
        "cargo:rustc-link-search=native={}",
        dir.join("lib").display()
    );
    let names = |kind: &str| -> Vec<String> {
        manifest["link"][kind]
            .as_array()
            .unwrap_or_else(|| panic!("kinora-ffmpeg.json: link.{kind} missing"))
            .iter()
            .map(|v| v.as_str().expect("library name").to_owned())
            .collect()
    };
    for lib in names("static") {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    for lib in names("system") {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
}
