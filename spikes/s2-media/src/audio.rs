//! A minimal export mixer: decode every input, resample to 48 kHz stereo
//! float, sum with gains, encode AAC. Packets are produced up front and
//! interleaved with video by the muxer loop.

use std::time::Instant;

use ffmpeg::{
    ChannelLayout, Dictionary, Packet, Rational, codec, encoder, format, frame, media,
    software::resampling,
};
use ffmpeg_next as ffmpeg;

const RATE: u32 = 48_000;

pub struct AudioTrack {
    params: codec::Parameters,
    tb: Rational,
    packets: Vec<Packet>,
    pub mix_seconds: f64,
    pub encode_wall_s: f64,
}

/// Decodes `path`'s best audio stream to planar stereo f32 at 48 kHz.
fn decode_planar(path: &str, max_s: f64) -> Result<[Vec<f32>; 2], String> {
    let mut ictx = format::input(path).map_err(|e| format!("{path}: {e}"))?;
    let st = ictx
        .streams()
        .best(media::Type::Audio)
        .ok_or_else(|| format!("{path}: no audio"))?;
    let index = st.index();
    let mut dec = codec::context::Context::from_parameters(st.parameters())
        .map_err(|e| e.to_string())?
        .decoder()
        .audio()
        .map_err(|e| e.to_string())?;
    let mut rs = resampling::Context::get(
        dec.format(),
        dec.channel_layout(),
        dec.rate(),
        format::Sample::F32(format::sample::Type::Planar),
        ChannelLayout::default(2),
        RATE,
    )
    .map_err(|e| e.to_string())?;
    let limit = (max_s * f64::from(RATE)) as usize;
    let mut out = [Vec::new(), Vec::new()];
    let mut decoded = frame::Audio::empty();
    let take = |decoded: &frame::Audio,
                rs: &mut resampling::Context,
                out: &mut [Vec<f32>; 2]|
     -> Result<(), String> {
        let cap = decoded.samples() * RATE as usize / decoded.rate().max(1) as usize + 256;
        let mut conv = frame::Audio::new(
            format::Sample::F32(format::sample::Type::Planar),
            cap,
            ChannelLayout::default(2),
        );
        rs.run(decoded, &mut conv).map_err(|e| e.to_string())?;
        let n = conv.samples();
        for (c, buf) in out.iter_mut().enumerate() {
            buf.extend_from_slice(&conv.plane::<f32>(c)[..n]);
        }
        Ok(())
    };
    for (s, p) in ictx.packets() {
        if s.index() != index {
            continue;
        }
        dec.send_packet(&p).map_err(|e| e.to_string())?;
        while dec.receive_frame(&mut decoded).is_ok() {
            take(&decoded, &mut rs, &mut out)?;
        }
        if out[0].len() >= limit {
            break;
        }
    }
    for buf in &mut out {
        buf.truncate(limit);
    }
    Ok(out)
}

impl AudioTrack {
    /// Mixes `inputs` (path, gain) over `seconds` and encodes AAC at `bitrate`.
    pub fn build(inputs: &[(&str, f32)], seconds: f64, bitrate: usize) -> Result<Self, String> {
        let t0 = Instant::now();
        let n = (seconds * f64::from(RATE)) as usize;
        let mut mix = [vec![0f32; n], vec![0f32; n]];
        for (path, gain) in inputs {
            let src = decode_planar(path, seconds)?;
            for c in 0..2 {
                for (m, s) in mix[c].iter_mut().zip(&src[c]) {
                    *m += s * gain;
                }
            }
        }
        for c in &mut mix {
            for s in c.iter_mut() {
                *s = s.clamp(-1.0, 1.0);
            }
        }
        let mix_seconds = t0.elapsed().as_secs_f64();

        let t1 = Instant::now();
        let codec = encoder::find(codec::Id::AAC).ok_or("no AAC encoder")?;
        let mut ctx = codec::context::Context::new_with_codec(codec)
            .encoder()
            .audio()
            .map_err(|e| e.to_string())?;
        ctx.set_rate(RATE as i32);
        ctx.set_channel_layout(ChannelLayout::default(2));
        ctx.set_format(format::Sample::F32(format::sample::Type::Planar));
        ctx.set_bit_rate(bitrate);
        ctx.set_time_base(Rational(1, RATE as i32));
        unsafe {
            (*ctx.as_mut_ptr()).flags |= ffmpeg::ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
        }
        let mut enc = ctx
            .open_with(Dictionary::new())
            .map_err(|e| format!("open aac: {e}"))?;
        let frame_size = enc.frame_size().max(1024) as usize;
        let mut packets = Vec::new();
        let mut pkt = Packet::empty();
        let mut pos = 0usize;
        while pos < n {
            let len = frame_size.min(n - pos);
            let mut f = frame::Audio::new(
                format::Sample::F32(format::sample::Type::Planar),
                len,
                ChannelLayout::default(2),
            );
            f.set_rate(RATE);
            for c in 0..2 {
                f.plane_mut::<f32>(c)[..len].copy_from_slice(&mix[c][pos..pos + len]);
            }
            f.set_pts(Some(pos as i64));
            enc.send_frame(&f).map_err(|e| e.to_string())?;
            while enc.receive_packet(&mut pkt).is_ok() {
                packets.push(pkt.clone());
                pkt = Packet::empty();
            }
            pos += len;
        }
        enc.send_eof().ok();
        while enc.receive_packet(&mut pkt).is_ok() {
            packets.push(pkt.clone());
            pkt = Packet::empty();
        }
        let params = codec::Parameters::from(&enc);
        Ok(Self {
            params,
            tb: Rational(1, RATE as i32),
            packets,
            mix_seconds,
            encode_wall_s: t1.elapsed().as_secs_f64(),
        })
    }

    pub fn stream_params(&self) -> (codec::Parameters, Rational) {
        (self.params.clone(), self.tb)
    }

    pub fn packets(&self) -> Result<Vec<Packet>, String> {
        Ok(self.packets.clone())
    }
}
