//! Demuxing only: packets leave Rust still encoded and are decoded by
//! WebCodecs in the WebView (ADR-0003, option A).
//!
//! Wire format of one `read_packets` response (little endian), repeated:
//!   u32 payload length | u8 flags | i64 pts (µs) | i64 duration (µs) | payload
//! flags: bit 0 = key frame, bit 1 = end of stream (payload empty).

use ffmpeg::{Rational, codec, format, media};
use ffmpeg_next as ffmpeg;
use serde::Serialize;

const US: Rational = Rational(1, 1_000_000);
pub const FLAG_KEY: u8 = 1;
pub const FLAG_EOS: u8 = 2;
pub const HEADER_BYTES: usize = 4 + 1 + 8 + 8;

/// What the WebView needs to configure a `VideoDecoder`.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: u32,
    pub path: String,
    /// WebCodecs codec string, or `None` when the codec has no WebCodecs
    /// mapping (ProRes, DNxHR, ...): those go through a proxy.
    pub codec: Option<String>,
    pub codec_name: String,
    pub coded_width: u32,
    pub coded_height: u32,
    /// avcC / hvcC / av1C record (`description` in VideoDecoderConfig).
    pub description: Option<Vec<u8>>,
    pub duration_us: i64,
    pub frame_rate: f64,
    pub frame_count: i64,
    pub color_space: String,
    pub color_range: String,
    pub has_audio: bool,
}

pub struct Source {
    input: format::context::Input,
    stream: usize,
    time_base: Rational,
    eos: bool,
    pub info: SourceInfo,
}

impl Source {
    pub fn open(id: u32, path: &str) -> Result<Self, String> {
        let input = format::input(path).map_err(|e| format!("open {path}: {e}"))?;
        let stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or_else(|| format!("{path}: no video stream"))?;
        let index = stream.index();
        let time_base = stream.time_base();
        let params = stream.parameters();
        let (extradata, width, height, color_space, color_range) = unsafe {
            let p = &*params.as_ptr();
            let extra = (!p.extradata.is_null() && p.extradata_size > 0).then(|| {
                std::slice::from_raw_parts(p.extradata, p.extradata_size as usize).to_vec()
            });
            (
                extra,
                p.width as u32,
                p.height as u32,
                p.color_space as i32,
                p.color_range as i32,
            )
        };
        let id_codec = params.id();
        let codec_name = format!("{id_codec:?}").to_lowercase();
        let (codec, description) = codec_string(id_codec, extradata.as_deref());
        let rate = stream.avg_frame_rate();
        let frame_rate = if rate.denominator() != 0 {
            f64::from(rate)
        } else {
            0.0
        };
        let duration_us = if input.duration() > 0 {
            input.duration()
        } else {
            0
        };
        let has_audio = input.streams().best(media::Type::Audio).is_some();
        let frame_count = stream.frames();

        let info = SourceInfo {
            id,
            path: path.to_owned(),
            codec,
            codec_name,
            coded_width: width,
            coded_height: height,
            description,
            duration_us,
            frame_rate,
            frame_count,
            color_space: color_space_name(color_space).to_owned(),
            color_range: if color_range == 1 {
                "tv".into()
            } else if color_range == 2 {
                "pc".into()
            } else {
                "unspecified".into()
            },
            has_audio,
        };
        Ok(Self {
            input,
            stream: index,
            time_base,
            eos: false,
            info,
        })
    }

    /// Appends up to `max_packets` / about `max_bytes` of video packets to `out`.
    pub fn read_packets(&mut self, max_bytes: usize, max_packets: usize, out: &mut Vec<u8>) {
        let mut count = 0;
        let start = out.len();
        while count < max_packets && out.len() - start < max_bytes {
            if self.eos {
                push_record(out, FLAG_EOS, 0, 0, &[]);
                return;
            }
            let mut packet = codec::packet::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => {
                    self.eos = true;
                    continue;
                }
                Err(_) => {
                    // Corrupt packet: skip it, as a player would.
                    continue;
                }
            }
            if packet.stream() != self.stream {
                continue;
            }
            let Some(data) = packet.data() else { continue };
            let ts = packet.pts().or(packet.dts()).unwrap_or(0);
            let pts_us = ts.rescale_to_us(self.time_base);
            let dur_us = packet.duration().rescale_to_us(self.time_base);
            let flags = if packet.is_key() { FLAG_KEY } else { 0 };
            push_record(out, flags, pts_us, dur_us, data);
            count += 1;
        }
    }

    /// Seeks to the key frame at or before `t_us`.
    pub fn seek(&mut self, t_us: i64) -> Result<(), String> {
        self.eos = false;
        self.input
            .seek(t_us, ..t_us)
            .map_err(|e| format!("seek: {e}"))
    }
}

trait RescaleUs {
    fn rescale_to_us(self, tb: Rational) -> i64;
}

impl RescaleUs for i64 {
    fn rescale_to_us(self, tb: Rational) -> i64 {
        use ffmpeg::Rescale;
        self.rescale(tb, US)
    }
}

fn push_record(out: &mut Vec<u8>, flags: u8, pts_us: i64, dur_us: i64, payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.push(flags);
    out.extend_from_slice(&pts_us.to_le_bytes());
    out.extend_from_slice(&dur_us.to_le_bytes());
    out.extend_from_slice(payload);
}

fn color_space_name(v: i32) -> &'static str {
    match v {
        1 => "bt709",
        5 | 6 => "bt601",
        9 | 10 => "bt2020",
        _ => "unspecified",
    }
}

/// Builds the WebCodecs codec string (RFC 6381 style) from the container's
/// decoder configuration record.
fn codec_string(id: codec::Id, extra: Option<&[u8]>) -> (Option<String>, Option<Vec<u8>>) {
    match (id, extra) {
        // avcC: [1] profile, [2] constraint flags, [3] level.
        (codec::Id::H264, Some(e)) if e.len() > 4 && e[0] == 1 => (
            Some(format!("avc1.{:02x}{:02x}{:02x}", e[1], e[2], e[3])),
            Some(e.to_vec()),
        ),
        // Annex B (no avcC): WebCodecs then expects Annex B data and no description.
        (codec::Id::H264, _) => (Some("avc1.640033".into()), None),
        (codec::Id::HEVC, Some(e)) if e.len() >= 23 && e[0] == 1 => {
            (Some(hevc_string(e)), Some(e.to_vec()))
        }
        (codec::Id::HEVC, _) => (Some("hev1.1.6.L153.B0".into()), None),
        (codec::Id::AV1, Some(e)) if e.len() >= 4 && e[0] & 0x80 != 0 => {
            (Some(av1_string(e)), Some(e.to_vec()))
        }
        (codec::Id::AV1, _) => (Some("av01.0.08M.08".into()), None),
        (codec::Id::VP9, _) => (Some("vp09.00.40.08".into()), None),
        (codec::Id::VP8, _) => (Some("vp8".into()), None),
        _ => (None, None),
    }
}

/// hvcC -> "hvc1.<space><profile>.<compat reversed hex>.<tier><level>.<constraints>".
fn hevc_string(e: &[u8]) -> String {
    let space = ["", "A", "B", "C"][usize::from(e[1] >> 6)];
    let tier = if e[1] & 0x20 != 0 { 'H' } else { 'L' };
    let profile = e[1] & 0x1f;
    let compat = u32::from_be_bytes([e[2], e[3], e[4], e[5]]).reverse_bits();
    let level = e[12];
    let mut constraints: Vec<u8> = e[6..12].to_vec();
    while constraints.last() == Some(&0) {
        constraints.pop();
    }
    let mut s = format!("hvc1.{space}{profile}.{compat:X}.{tier}{level}");
    for b in constraints {
        s.push_str(&format!(".{b:X}"));
    }
    s
}

/// av1C -> "av01.<profile>.<level><tier>.<bitdepth>".
fn av1_string(e: &[u8]) -> String {
    let profile = e[1] >> 5;
    let level = e[1] & 0x1f;
    let tier = if e[2] & 0x80 != 0 { 'H' } else { 'M' };
    let high = e[2] & 0x40 != 0;
    let twelve = e[2] & 0x20 != 0;
    let depth = if high && twelve {
        12
    } else if high {
        10
    } else {
        8
    };
    format!("av01.{profile}.{level:02}{tier}.{depth:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hevc_main10_level_5_1() {
        // Main 10, compat flag 2, tier main, level 153 (5.1), constraint byte 0x90.
        let mut e = vec![0u8; 23];
        e[0] = 1;
        e[1] = 2;
        e[2..6].copy_from_slice(&[0x20, 0, 0, 0]);
        e[6] = 0x90;
        e[12] = 153;
        assert_eq!(hevc_string(&e), "hvc1.2.4.L153.90");
    }

    #[test]
    fn av1_main_8bit() {
        assert_eq!(av1_string(&[0x81, 0x08, 0x0c, 0x00]), "av01.0.08M.08");
    }
}

/// Leak localisation: repeat what one benchmark run does to the demuxer,
/// without IPC or WebView, and watch the resident set.
#[cfg(test)]
pub(crate) mod leak {
    use super::*;
    use sysinfo::{ProcessesToUpdate, System};

    pub fn rss_mb() -> f64 {
        let mut sys = System::new();
        let pid = sysinfo::get_current_pid().unwrap();
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        sys.process(pid)
            .map(|p| p.memory() as f64 / 1048576.0)
            .unwrap_or(0.0)
    }

    pub fn media(name: &str) -> String {
        format!(
            "{}/../../../tests/media/s1/{name}",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    #[test]
    #[ignore = "manual: cargo test --release -- --ignored --nocapture leak"]
    fn leak_demux() {
        ffmpeg::init().unwrap();
        let mut out = Vec::new();
        let mut samples = Vec::new();
        for i in 0..40 {
            let mut src = Source::open(1, &media("1080p30-h264.mp4")).unwrap();
            for _ in 0..200 {
                out.clear();
                src.read_packets(600_000, 8, &mut out);
            }
            for k in 0..5 {
                src.seek(5_000_000 * k).unwrap();
                out.clear();
                src.read_packets(4_000_000, 32, &mut out);
            }
            drop(src);
            samples.push(rss_mb());
            if i % 10 == 9 {
                println!("demux iteration {:>2}: rss {:.1} MB", i + 1, rss_mb());
            }
        }
        let growth = samples[39] - samples[9];
        println!("demux rss growth iterations 10..40: {growth:.1} MB");
        assert!(
            growth < 20.0,
            "demux leaks {growth:.1} MB over 30 iterations"
        );
    }
}
