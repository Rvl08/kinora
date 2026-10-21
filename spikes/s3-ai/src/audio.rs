//! WAV I/O and resampling for the speech tests.

use std::path::Path;

/// Mono f32 samples and their rate.
pub struct Pcm {
    pub rate: u32,
    pub samples: Vec<f32>,
}

impl Pcm {
    pub fn seconds(&self) -> f64 {
        self.samples.len() as f64 / f64::from(self.rate)
    }
}

pub fn read_wav(path: &Path) -> Result<Pcm, String> {
    let mut r = hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = r.spec();
    let ch = usize::from(spec.channels.max(1));
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 * scale)
                .collect()
        }
    };
    let samples = raw
        .chunks(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect();
    Ok(Pcm {
        rate: spec.sample_rate,
        samples,
    })
}

pub fn write_wav(path: &Path, pcm: &Pcm) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: pcm.rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in &pcm.samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

/// Windowed-sinc resampler (Hann, 32 taps per side). Plenty for speech
/// recognition input; not meant for music.
pub fn resample(pcm: &Pcm, to: u32) -> Pcm {
    if pcm.rate == to {
        return Pcm {
            rate: to,
            samples: pcm.samples.clone(),
        };
    }
    let ratio = f64::from(to) / f64::from(pcm.rate);
    let cutoff = ratio.min(1.0) * 0.95;
    let taps = 32i64;
    let n_out = (pcm.samples.len() as f64 * ratio) as usize;
    let mut out = Vec::with_capacity(n_out);
    for i in 0..n_out {
        let t = i as f64 / ratio;
        let center = t.floor() as i64;
        let mut acc = 0.0f64;
        let mut norm = 0.0f64;
        for k in (center - taps + 1)..=(center + taps) {
            if k < 0 || k as usize >= pcm.samples.len() {
                continue;
            }
            let x = t - k as f64;
            let sinc = if x.abs() < 1e-9 {
                1.0
            } else {
                (std::f64::consts::PI * x * cutoff).sin() / (std::f64::consts::PI * x)
            };
            let w = 0.5 + 0.5 * (std::f64::consts::PI * x / taps as f64).cos();
            let h = sinc * w;
            acc += f64::from(pcm.samples[k as usize]) * h;
            norm += h;
        }
        out.push(if norm.abs() > 1e-12 {
            (acc / norm) as f32
        } else {
            0.0
        });
    }
    Pcm {
        rate: to,
        samples: out,
    }
}
