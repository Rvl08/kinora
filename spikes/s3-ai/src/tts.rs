//! Kokoro-82M through ONNX Runtime, fed by Kinora's own G2P (no espeak-ng).

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use ndarray::{Array1, Array2};
use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;
use serde_json::Value;

use crate::audio::Pcm;
use crate::ortx::{self, Device, err};

pub const RATE: u32 = 24_000;

pub struct Kokoro {
    session: Session,
    vocab: HashMap<char, i64>,
    names: (String, String, String),
    pub load_ms: f64,
    pub signature: (Vec<String>, Vec<String>),
}

pub struct Synthesis {
    pub pcm: Pcm,
    pub dropped: Vec<char>,
    pub wall_s: f64,
}

/// A voice pack: 510 style vectors of 256 floats, indexed by token count.
pub struct Voice(Vec<f32>);

impl Voice {
    pub fn load(path: &Path) -> Result<Self, String> {
        let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if b.len() % (256 * 4) != 0 {
            return Err(format!(
                "{}: {} bytes is not a multiple of 256 floats",
                path.display(),
                b.len()
            ));
        }
        Ok(Self(
            b.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
        ))
    }
    fn style(&self, tokens: usize) -> Vec<f32> {
        let rows = self.0.len() / 256;
        let r = tokens.min(rows - 1);
        self.0[r * 256..(r + 1) * 256].to_vec()
    }
}

impl Kokoro {
    pub fn load(
        env: &Environment,
        model: &Path,
        tokenizer: &Path,
        device: Device,
    ) -> Result<Self, String> {
        let text = std::fs::read_to_string(tokenizer).map_err(|e| e.to_string())?;
        let tj: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let vocab_v = tj["model"]["vocab"]
            .as_object()
            .ok_or("tokenizer.json: no model.vocab")?;
        let mut vocab = HashMap::new();
        for (k, v) in vocab_v {
            let mut it = k.chars();
            if let (Some(c), None) = (it.next(), it.next()) {
                vocab.insert(c, v.as_i64().unwrap_or(0));
            }
        }
        let t0 = Instant::now();
        let session = ortx::session(env, model, device, 8)?;
        let load_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let signature = ortx::signature(&session);
        let ins: Vec<String> = session
            .inputs()
            .iter()
            .map(|o| o.name().to_owned())
            .collect();
        let pick = |k: &str, i: usize| {
            ins.iter()
                .find(|n| n.contains(k))
                .cloned()
                .unwrap_or_else(|| ins[i].clone())
        };
        let names = (pick("input", 0), pick("style", 1), pick("speed", 2));
        Ok(Self {
            session,
            vocab,
            names,
            load_ms,
            signature,
        })
    }

    pub fn synthesize(
        &mut self,
        phonemes: &str,
        voice: &Voice,
        speed: f32,
    ) -> Result<Synthesis, String> {
        let mut ids = vec![0i64];
        let mut dropped = Vec::new();
        for c in phonemes.chars() {
            match self.vocab.get(&c) {
                Some(&id) => ids.push(id),
                None => dropped.push(c),
            }
        }
        // The model's context is 512 tokens including the two pads.
        ids.truncate(511);
        ids.push(0);
        let tokens = ids.len() - 2;
        let style = voice.style(tokens);
        let t0 = Instant::now();
        let input = Tensor::from_array(Array2::from_shape_vec((1, ids.len()), ids).map_err(err)?)
            .map_err(err)?;
        let style = Tensor::from_array(Array2::from_shape_vec((1, 256), style).map_err(err)?)
            .map_err(err)?;
        let speed = Tensor::from_array(Array1::from_vec(vec![speed])).map_err(err)?;
        let (a, b, c) = (
            self.names.0.as_str(),
            self.names.1.as_str(),
            self.names.2.as_str(),
        );
        let out = self
            .session
            .run(ort::inputs![a => input, b => style, c => speed])
            .map_err(err)?;
        let samples: Vec<f32> = out[0]
            .try_extract_array::<f32>()
            .map_err(err)?
            .iter()
            .copied()
            .collect();
        let wall_s = t0.elapsed().as_secs_f64();
        dropped.sort_unstable();
        dropped.dedup();
        Ok(Synthesis {
            pcm: Pcm {
                rate: RATE,
                samples,
            },
            dropped,
            wall_s,
        })
    }
}
