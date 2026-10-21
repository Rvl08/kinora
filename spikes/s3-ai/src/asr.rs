//! Speech recognition with whisper.cpp (whisper-rs): text, word timestamps
//! (token heuristic and DTW), speed.

use std::path::Path;
use std::time::Instant;

use serde::Serialize;
use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext,
    WhisperContextParameters,
};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    pub text: String,
    /// Start from whisper's token timestamps (ms).
    pub start_ms: f64,
    /// Start from DTW token alignment (ms), when enabled.
    pub start_dtw_ms: Option<f64>,
    /// DTW time of the token before the word (ms): if DTW marks token ends,
    /// this is the better start estimate.
    pub start_dtw_prev_ms: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub text: String,
    pub words: Vec<Word>,
    pub audio_s: f64,
    pub wall_s: f64,
    pub realtime_factor: f64,
}

pub struct Asr {
    ctx: WhisperContext,
    pub load_s: f64,
    pub dtw: bool,
}

fn preset(model_id: &str) -> Option<DtwModelPreset> {
    if model_id.contains("large-v3-turbo") {
        Some(DtwModelPreset::LargeV3Turbo)
    } else if model_id.contains("small") {
        Some(DtwModelPreset::Small)
    } else if model_id.contains("base") {
        Some(DtwModelPreset::Base)
    } else {
        None
    }
}

impl Asr {
    pub fn load(model: &Path, model_id: &str, gpu: bool, dtw: bool) -> Result<Self, String> {
        let t0 = Instant::now();
        let mut cp = WhisperContextParameters::default();
        cp.use_gpu(gpu);
        // DTW alignment needs the attention weights, which flash attention does not keep.
        cp.flash_attn(false);
        let dtw = dtw && preset(model_id).is_some();
        if dtw {
            cp.dtw_parameters(DtwParameters {
                mode: DtwMode::ModelPreset {
                    model_preset: preset(model_id).unwrap(),
                },
                ..Default::default()
            });
        }
        let ctx = WhisperContext::new_with_params(model, cp)
            .map_err(|e| format!("load {}: {e}", model.display()))?;
        Ok(Self {
            ctx,
            load_s: t0.elapsed().as_secs_f64(),
            dtw,
        })
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        lang: &str,
        threads: i32,
    ) -> Result<Transcript, String> {
        let mut state = self.ctx.create_state().map_err(|e| e.to_string())?;
        let mut p = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });
        p.set_language(Some(lang));
        p.set_n_threads(threads);
        p.set_token_timestamps(true);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_special(false);
        p.set_print_timestamps(false);
        let t0 = Instant::now();
        state.full(p, samples).map_err(|e| e.to_string())?;
        let wall = t0.elapsed().as_secs_f64();

        let eot = self.ctx.token_eot();
        let mut words: Vec<(Vec<u8>, f64, Option<f64>, Option<f64>)> = Vec::new();
        let mut prev_dtw: Option<f64> = None;
        let mut text = String::new();
        for seg in state.as_iter() {
            text.push_str(&seg.to_str_lossy().map_err(|e| e.to_string())?);
            for i in 0..seg.n_tokens() {
                let Some(tok) = seg.get_token(i) else {
                    continue;
                };
                let d = tok.token_data();
                if d.id >= eot {
                    continue; // special and timestamp tokens
                }
                let bytes = tok.to_bytes().map_err(|e| e.to_string())?.to_vec();
                let starts_word = bytes.first() == Some(&b' ') || words.is_empty();
                // Whisper times are in 10 ms units.
                let t = d.t0 as f64 * 10.0;
                let td = (self.dtw && d.t_dtw >= 0).then(|| d.t_dtw as f64 * 10.0);
                if starts_word {
                    words.push((bytes, t, td, prev_dtw.or(td.map(|_| 0.0))));
                } else if let Some(last) = words.last_mut() {
                    last.0.extend_from_slice(&bytes);
                }
                prev_dtw = td;
            }
        }
        let words = words
            .into_iter()
            .map(|(b, t, td, tp)| Word {
                text: String::from_utf8_lossy(&b).trim().to_owned(),
                start_ms: t,
                start_dtw_ms: td,
                start_dtw_prev_ms: tp,
            })
            .filter(|w| !w.text.is_empty())
            .collect();
        let audio_s = samples.len() as f64 / 16_000.0;
        Ok(Transcript {
            text: text.trim().to_owned(),
            words,
            audio_s,
            wall_s: wall,
            realtime_factor: wall / audio_s,
        })
    }
}
