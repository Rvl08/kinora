//! Audio playback through cpal (WASAPI) and the master clock (ADR-0003 §4).
//!
//! The clock is the media time of the sample the device is playing *now*:
//! every data callback records which sample frame it starts with and when
//! that frame will reach the DAC (`playback` timestamp), and `clock_us`
//! extrapolates from the latest record. Video follows this clock.
//!
//! The cpal stream is not `Send` on every host, so it lives on its own
//! thread together with the decoder; commands arrive over a channel.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering::*};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ffmpeg::{ChannelLayout, Rational, Rescale, format, frame, media, software::resampling};
use ffmpeg_next as ffmpeg;
use serde::Serialize;

enum Cmd {
    Load(String),
    Play,
    Pause,
    Seek(i64),
}

#[derive(Default)]
struct Shared {
    /// True once a device stream is running; otherwise the clock is wall time.
    device: AtomicBool,
    /// True while an audio track is loaded; a file without one also uses wall time.
    source: AtomicBool,
    playing: AtomicBool,
    /// Media time (µs) of sample frame 0 since the last seek.
    base_us: AtomicI64,
    rate: AtomicU32,
    /// Frames handed to the device before the latest callback.
    cb_frames: AtomicU64,
    /// Frames handed in the latest callback.
    cb_len: AtomicU64,
    /// When the latest callback's first frame reaches the DAC (ns since epoch).
    cb_play_ns: AtomicU64,
    latency_us: AtomicI64,
    underruns: AtomicU64,
    /// Set by the producer to make the callback drop queued samples.
    flush: AtomicBool,
    /// Wall-clock fallback: media time at play, and when play started.
    wall_base_us: AtomicI64,
    wall_start_ns: AtomicU64,
}

pub struct AudioEngine {
    tx: Sender<Cmd>,
    shared: Arc<Shared>,
    epoch: Instant,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    pub media_us: i64,
    pub playing: bool,
    pub device: bool,
    pub latency_us: i64,
    pub underruns: u64,
    pub rate: u32,
}

impl AudioEngine {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let epoch = Instant::now();
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || audio_thread(rx, thread_shared, epoch))
            .expect("spawn audio thread");
        Self { tx, shared, epoch }
    }

    pub fn load(&self, path: String) {
        let _ = self.tx.send(Cmd::Load(path));
    }
    pub fn play(&self) {
        let s = &self.shared;
        s.wall_base_us.store(self.clock().media_us, SeqCst);
        s.wall_start_ns
            .store(self.epoch.elapsed().as_nanos() as u64, SeqCst);
        let _ = self.tx.send(Cmd::Play);
        s.playing.store(true, SeqCst);
    }
    pub fn pause(&self) {
        let now = self.clock().media_us;
        self.shared.playing.store(false, SeqCst);
        self.shared.wall_base_us.store(now, SeqCst);
        let _ = self.tx.send(Cmd::Pause);
    }
    pub fn seek(&self, t_us: i64) {
        self.shared.wall_base_us.store(t_us, SeqCst);
        self.shared
            .wall_start_ns
            .store(self.epoch.elapsed().as_nanos() as u64, SeqCst);
        let _ = self.tx.send(Cmd::Seek(t_us));
    }

    pub fn clock(&self) -> Clock {
        let s = &self.shared;
        let playing = s.playing.load(SeqCst);
        let device = s.device.load(SeqCst) && s.source.load(SeqCst);
        let now_ns = self.epoch.elapsed().as_nanos() as i64;
        let media_us = if device {
            let rate = i64::from(s.rate.load(SeqCst).max(1));
            let base = s.base_us.load(SeqCst);
            let frames = s.cb_frames.load(SeqCst) as i64;
            let len = s.cb_len.load(SeqCst) as i64;
            let play_ns = s.cb_play_ns.load(SeqCst) as i64;
            // Frames played since the latest callback's first frame hit the DAC,
            // clamped to what that callback actually supplied.
            let since = if playing {
                ((now_ns - play_ns).max(0) * rate / 1_000_000_000).min(len)
            } else {
                0
            };
            let since = if play_ns == 0 { 0 } else { since };
            base + (frames + since) * 1_000_000 / rate
        } else if playing {
            s.wall_base_us.load(SeqCst) + (now_ns - s.wall_start_ns.load(SeqCst) as i64) / 1000
        } else {
            s.wall_base_us.load(SeqCst)
        };
        Clock {
            media_us,
            playing,
            device,
            latency_us: s.latency_us.load(SeqCst),
            underruns: s.underruns.load(SeqCst),
            rate: s.rate.load(SeqCst),
        }
    }
}

struct Decoding {
    input: format::context::Input,
    stream: usize,
    time_base: Rational,
    decoder: ffmpeg::decoder::Audio,
    resampler: resampling::Context,
    /// Drop samples before this media time (µs) after a seek.
    skip_until_us: i64,
    eos: bool,
}

impl Decoding {
    fn open(path: &str, rate: u32) -> Result<Self, String> {
        let input = format::input(path).map_err(|e| e.to_string())?;
        let stream = input
            .streams()
            .best(media::Type::Audio)
            .ok_or("no audio stream")?;
        let index = stream.index();
        let time_base = stream.time_base();
        let ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| e.to_string())?;
        let decoder = ctx.decoder().audio().map_err(|e| e.to_string())?;
        let resampler = resampling::Context::get(
            decoder.format(),
            decoder.channel_layout(),
            decoder.rate(),
            format::Sample::F32(format::sample::Type::Packed),
            ChannelLayout::default(2),
            rate,
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            input,
            stream: index,
            time_base,
            decoder,
            resampler,
            skip_until_us: 0,
            eos: false,
        })
    }

    fn seek(&mut self, t_us: i64) {
        let _ = self.input.seek(t_us, ..t_us);
        self.decoder.flush();
        self.skip_until_us = t_us;
        self.eos = false;
    }

    /// Decodes one packet's worth of audio as interleaved stereo f32.
    fn next_samples(&mut self, out_rate: u32, out: &mut Vec<f32>) {
        if self.eos {
            return;
        }
        // A fresh packet per read: ffmpeg-next's Packet::read calls av_read_frame
        // without unreferencing the previous payload, so reusing one packet while
        // skipping the (interleaved) video packets leaks every skipped payload.
        let packet = loop {
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) if packet.stream() == self.stream => break packet,
                Ok(()) => continue,
                Err(ffmpeg::Error::Eof) => {
                    self.eos = true;
                    return;
                }
                Err(_) => continue,
            }
        };
        if self.decoder.send_packet(&packet).is_err() {
            return;
        }
        let mut decoded = frame::Audio::empty();
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            let pts_us = decoded
                .pts()
                .unwrap_or(0)
                .rescale(self.time_base, Rational(1, 1_000_000));
            let capacity =
                decoded.samples() * out_rate as usize / decoded.rate().max(1) as usize + 256;
            let mut converted = frame::Audio::new(
                format::Sample::F32(format::sample::Type::Packed),
                capacity,
                ChannelLayout::default(2),
            );
            if self.resampler.run(&decoded, &mut converted).is_err() {
                continue;
            }
            let n = converted.samples() * 2;
            let bytes = &converted.data(0)[..n * 4];
            let samples: &[f32] = bytemuck::cast_slice(bytes);
            // Trim the part before the seek target.
            let skip_frames = if pts_us < self.skip_until_us {
                ((self.skip_until_us - pts_us) * i64::from(out_rate) / 1_000_000) as usize
            } else {
                0
            };
            let skip = (skip_frames * 2).min(samples.len());
            out.extend_from_slice(&samples[skip..]);
        }
    }
}

fn audio_thread(rx: Receiver<Cmd>, shared: Arc<Shared>, epoch: Instant) {
    let host = cpal::default_host();
    let device = host.default_output_device();
    let config = device.as_ref().and_then(|d| d.default_output_config().ok());
    let Some((device, config)) = device.zip(config) else {
        eprintln!("audio: no output device; clock falls back to wall time");
        return wall_clock_only(rx);
    };
    let rate = config.sample_rate();
    let channels = usize::from(config.channels());
    shared.rate.store(rate, SeqCst);

    // ~1 s of interleaved device-channel samples.
    let (mut producer, mut consumer) = rtrb::RingBuffer::<f32>::new(rate as usize * channels);
    let cb_shared = shared.clone();
    let stream = device.build_output_stream(
        config.config(),
        move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
            let s = &cb_shared;
            if s.flush.swap(false, SeqCst) {
                let n = consumer.slots();
                if let Ok(chunk) = consumer.read_chunk(n) {
                    chunk.commit_all();
                }
            }
            let ts = info.timestamp();
            let latency = ts.playback.saturating_duration_since(ts.callback);
            s.latency_us.store(latency.as_micros() as i64, SeqCst);
            if !s.playing.load(SeqCst) {
                data.fill(0.0);
                return;
            }
            let available = consumer.slots().min(data.len());
            if let Ok(chunk) = consumer.read_chunk(available) {
                let (a, b) = chunk.as_slices();
                data[..a.len()].copy_from_slice(a);
                data[a.len()..a.len() + b.len()].copy_from_slice(b);
                chunk.commit_all();
            }
            if available < data.len() {
                data[available..].fill(0.0);
                s.underruns.fetch_add(1, SeqCst);
            }
            let played = (available / channels) as u64;
            // The first frame of this buffer plays `latency` from now.
            let play_at = epoch.elapsed() + latency;
            let prev = s.cb_frames.load(SeqCst) + s.cb_len.load(SeqCst);
            s.cb_frames.store(prev, SeqCst);
            s.cb_len.store(played, SeqCst);
            s.cb_play_ns.store(play_at.as_nanos() as u64, SeqCst);
        },
        |e| eprintln!("audio stream error: {e}"),
        None,
    );
    let Ok(stream) = stream else {
        eprintln!("audio: cannot open an f32 stream; clock falls back to wall time");
        return wall_clock_only(rx);
    };
    let _ = stream.play();
    shared.device.store(true, SeqCst);

    let mut decoding: Option<Decoding> = None;
    let mut pending: Vec<f32> = Vec::new();
    let mut stereo: Vec<f32> = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(4)) {
            Ok(Cmd::Load(path)) => {
                decoding = Decoding::open(&path, rate)
                    .map_err(|e| eprintln!("audio: {e}"))
                    .ok();
                shared.source.store(decoding.is_some(), SeqCst);
                reset(&shared, 0, &mut pending);
            }
            Ok(Cmd::Play) => shared.playing.store(true, SeqCst),
            Ok(Cmd::Pause) => shared.playing.store(false, SeqCst),
            Ok(Cmd::Seek(t)) => {
                if let Some(d) = decoding.as_mut() {
                    d.seek(t);
                }
                reset(&shared, t, &mut pending);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        // Keep the ring about half full.
        let Some(d) = decoding.as_mut() else { continue };
        while producer.slots() > rate as usize * channels / 2 {
            if pending.is_empty() {
                stereo.clear();
                d.next_samples(rate, &mut stereo);
                if stereo.is_empty() {
                    break;
                }
                // Stereo -> device channel count.
                for pair in stereo.chunks_exact(2) {
                    for c in 0..channels {
                        pending.push(if c < 2 { pair[c] } else { 0.0 });
                    }
                }
            }
            let n = pending.len().min(producer.slots());
            if let Ok(mut chunk) = producer.write_chunk(n) {
                let (a, b) = chunk.as_mut_slices();
                let split = a.len();
                a.copy_from_slice(&pending[..split]);
                b.copy_from_slice(&pending[split..split + b.len()]);
                chunk.commit_all();
                pending.drain(..n);
            }
        }
    }
}

fn reset(shared: &Shared, base_us: i64, pending: &mut Vec<f32>) {
    pending.clear();
    shared.flush.store(true, SeqCst);
    shared.base_us.store(base_us, SeqCst);
    shared.cb_frames.store(0, SeqCst);
    shared.cb_len.store(0, SeqCst);
    shared.cb_play_ns.store(0, SeqCst);
    // Give the callback one period to drop what was queued before the reset.
    std::thread::sleep(Duration::from_millis(15));
}

fn wall_clock_only(rx: Receiver<Cmd>) {
    while rx.recv().is_ok() {}
}

#[cfg(test)]
mod leak {
    use super::*;
    use crate::demux::leak::{media, rss_mb};

    #[test]
    #[ignore = "manual: cargo test --release -- --ignored --nocapture leak"]
    fn leak_audio_decode() {
        ffmpeg::init().unwrap();
        let mut samples = Vec::new();
        let mut out = Vec::new();
        for i in 0..40 {
            let mut d = Decoding::open(&media("1080p30-h264.mp4"), 48_000).unwrap();
            // ~20 s of audio, then a few seeks: one benchmark run's worth.
            for _ in 0..1000 {
                out.clear();
                d.next_samples(48_000, &mut out);
            }
            for k in 0..5 {
                d.seek(5_000_000 * k);
                out.clear();
                d.next_samples(48_000, &mut out);
            }
            drop(d);
            samples.push(rss_mb());
            if i % 10 == 9 {
                println!("audio iteration {:>2}: rss {:.1} MB", i + 1, rss_mb());
            }
        }
        let growth = samples[39] - samples[9];
        println!("audio rss growth iterations 10..40: {growth:.1} MB");
        assert!(
            growth < 20.0,
            "audio decode leaks {growth:.1} MB over 30 iterations"
        );
    }
}
