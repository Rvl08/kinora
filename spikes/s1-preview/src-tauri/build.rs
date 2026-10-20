use std::{env, fs, path::PathBuf};

fn main() {
    tauri_build::build();
    copy_ffmpeg_dlls();
}

/// Puts the LGPL FFmpeg DLLs next to the spike executable so it starts from
/// `cargo run` without touching PATH. FFMPEG_DIR comes from the repository's
/// .cargo/config.toml (vendor/ffmpeg-lgpl).
fn copy_ffmpeg_dlls() {
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    let Ok(ffmpeg) = env::var("FFMPEG_DIR") else {
        println!("cargo:warning=FFMPEG_DIR not set; FFmpeg DLLs not copied");
        return;
    };
    let bin = PathBuf::from(ffmpeg).join("bin");
    // OUT_DIR = target/<profile>/build/<pkg>-<hash>/out
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let Some(profile_dir) = out.ancestors().nth(3) else {
        return;
    };
    let Ok(entries) = fs::read_dir(&bin) else {
        println!("cargo:warning=no FFmpeg bin dir at {}", bin.display());
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dll"))
        {
            let target = profile_dir.join(entry.file_name());
            let fresh = fs::metadata(&target)
                .ok()
                .zip(entry.metadata().ok())
                .is_some_and(|(t, s)| t.len() == s.len());
            if !fresh {
                fs::copy(&path, &target).expect("copy FFmpeg DLL");
            }
        }
    }
}
