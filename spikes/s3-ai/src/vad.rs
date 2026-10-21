//! Silero VAD v5 through ONNX Runtime, with the official segmenter defaults.

use std::path::Path;
use std::time::Instant;

use ndarray::{Array1, Array2, Array3};
use ort::environment::Environment;
use ort::value::Tensor;
use serde::Serialize;

use crate::ortx::{self, Device, err};

const RATE: usize = 16_000;
const WINDOW: usize = 512;
const CONTEXT: usize = 64;

#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub start_ms: f64,
    pub end_ms: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VadRun {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub audio_s: f64,
    pub wall_ms: f64,
    pub realtime_factor: f64,
    pub segments: Vec<Segment>,
}

fn sr_value(scalar: bool) -> Result<ort::value::DynValue, String> {
    Ok(if scalar {
        Tensor::from_array(ndarray::arr0(RATE as i64))
            .map_err(err)?
            .into_dyn()
    } else {
        Tensor::from_array(Array1::from_vec(vec![RATE as i64]))
            .map_err(err)?
            .into_dyn()
    })
}

/// Silero v5 takes `sr` as a scalar; some exports want shape [1]. One trial
/// run on silence decides which this export accepts.
fn sr_is_scalar(
    session: &mut ort::session::Session,
    names: &(String, String, String),
) -> Result<bool, String> {
    let input = Tensor::from_array(Array2::<f32>::zeros((1, CONTEXT + WINDOW))).map_err(err)?;
    let st = Tensor::from_array(Array3::<f32>::zeros((2, 1, 128))).map_err(err)?;
    let ok = session
        .run(ort::inputs![names.0.as_str() => input, names.1.as_str() => st, names.2.as_str() => sr_value(true)?])
        .is_ok();
    Ok(ok)
}

/// Speech probability per 32 ms window.
fn probabilities(session: &mut ort::session::Session, samples: &[f32]) -> Result<Vec<f32>, String> {
    let names: Vec<String> = session
        .inputs()
        .iter()
        .map(|o| o.name().to_owned())
        .collect();
    let find = |k: &str| names.iter().find(|n| n.contains(k)).cloned();
    let n = (
        find("input").unwrap_or_else(|| names[0].clone()),
        find("state")
            .or_else(|| find("h"))
            .unwrap_or_else(|| names[1].clone()),
        find("sr").unwrap_or_else(|| names[2].clone()),
    );
    let scalar = sr_is_scalar(session, &n)?;

    let mut state = Array3::<f32>::zeros((2, 1, 128));
    let mut context = vec![0f32; CONTEXT];
    let mut probs = Vec::new();
    for chunk in samples.chunks(WINDOW) {
        let mut x = Vec::with_capacity(CONTEXT + WINDOW);
        x.extend_from_slice(&context);
        x.extend_from_slice(chunk);
        x.resize(CONTEXT + WINDOW, 0.0);
        context.copy_from_slice(&x[x.len() - CONTEXT..]);
        let input =
            Tensor::from_array(Array2::from_shape_vec((1, CONTEXT + WINDOW), x).map_err(err)?)
                .map_err(err)?;
        let st = Tensor::from_array(state.clone()).map_err(err)?;
        let out = session
            .run(ort::inputs![n.0.as_str() => input, n.1.as_str() => st, n.2.as_str() => sr_value(scalar)?])
            .map_err(err)?;
        let p = out[0].try_extract_array::<f32>().map_err(err)?;
        probs.push(*p.iter().next().unwrap_or(&0.0));
        let s = out[1].try_extract_array::<f32>().map_err(err)?;
        state = s
            .to_owned()
            .into_shape_with_order((2, 1, 128))
            .map_err(err)?;
    }
    Ok(probs)
}

/// The Silero reference segmenter: threshold 0.5, release at 0.35, speech at
/// least 250 ms, silence at least 100 ms to split, 30 ms padding.
fn segments(probs: &[f32], total_samples: usize) -> Vec<Segment> {
    let (thr, neg) = (0.5f32, 0.35f32);
    let min_speech = RATE * 250 / 1000;
    let min_silence = RATE * 100 / 1000;
    let pad = RATE * 30 / 1000;
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut in_speech = false;
    let mut start = 0usize;
    let mut silence_from: Option<usize> = None;
    for (i, &p) in probs.iter().enumerate() {
        let pos = i * WINDOW;
        if p >= thr {
            silence_from = None;
            if !in_speech {
                in_speech = true;
                start = pos;
            }
        } else if in_speech && p < neg {
            let s = *silence_from.get_or_insert(pos);
            if pos - s >= min_silence {
                if s - start >= min_speech {
                    out.push((start, s));
                }
                in_speech = false;
                silence_from = None;
            }
        }
    }
    if in_speech && total_samples - start >= min_speech {
        out.push((start, total_samples));
    }
    out.iter()
        .map(|&(s, e)| Segment {
            start_ms: s.saturating_sub(pad) as f64 * 1000.0 / RATE as f64,
            end_ms: (e + pad).min(total_samples) as f64 * 1000.0 / RATE as f64,
        })
        .collect()
}

pub fn run(env: &Environment, model: &Path, samples: &[f32]) -> Result<VadRun, String> {
    let mut session = ortx::session(env, model, Device::Cpu, 1)?;
    let (inputs, outputs) = ortx::signature(&session);
    let t0 = Instant::now();
    let probs = probabilities(&mut session, samples)?;
    let wall = t0.elapsed().as_secs_f64();
    let audio_s = samples.len() as f64 / RATE as f64;
    Ok(VadRun {
        inputs,
        outputs,
        audio_s,
        wall_ms: wall * 1000.0,
        realtime_factor: wall / audio_s,
        segments: segments(&probs, samples.len()),
    })
}
