//! Size baseline: the same crate and build settings without any FFmpeg
//! reference, so `ffmpeg-static-probe.exe - size-baseline.exe` is what the
//! static FFmpeg adds to an executable.

fn main() {
    println!("size baseline: no FFmpeg linked");
}
