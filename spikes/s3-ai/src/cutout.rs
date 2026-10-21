//! Background removal through ONNX Runtime (CPU / DirectML): BiRefNet-lite
//! (stock fixed-size export and the dynamic-size DeformConv export), IS-Net
//! and U2-Net-P, with quality measured against a reference model's masks.

use std::path::{Path, PathBuf};
use std::time::Instant;

use image::{GrayImage, Luma, RgbaImage, imageops::FilterType};
use ndarray::Array4;
use ort::environment::Environment;
use ort::value::Tensor;
use serde::Serialize;

use crate::ortx::{self, Device, err};

const IMAGENET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const IMAGENET_STD: [f32; 3] = [0.229, 0.224, 0.225];
/// Masks are compared on the image scaled so its longest side is this long.
const EVAL_SIDE: u32 = 1024;
/// Half-width of the band around the reference contour for the edge error.
const BAND: usize = 8;

#[derive(Clone, Copy)]
enum Post {
    /// Sigmoid when the output holds logits (BiRefNet exports differ).
    SigmoidIfLogits,
    /// Per-image min-max scaling (IS-Net, U2-Net outputs).
    MinMax,
}

/// How a model wants its input and how to read its output.
#[derive(Clone, Copy)]
pub struct Spec {
    pub size: u32,
    mean: [f32; 3],
    std: [f32; 3],
    post: Post,
}

/// Input size and normalisation per model id. `size` only applies to the
/// dynamic export (any multiple of 64).
pub fn spec(model_id: &str, size: Option<u32>) -> Spec {
    match model_id {
        "birefnet-lite-dyn" => Spec {
            size: size.unwrap_or(1024),
            mean: IMAGENET_MEAN,
            std: IMAGENET_STD,
            post: Post::SigmoidIfLogits,
        },
        "isnet-general" => Spec {
            size: 1024,
            mean: [0.5; 3],
            std: [1.0; 3],
            post: Post::MinMax,
        },
        "u2netp" => Spec {
            size: 320,
            mean: IMAGENET_MEAN,
            std: IMAGENET_STD,
            post: Post::MinMax,
        },
        _ => Spec {
            size: 1024,
            mean: IMAGENET_MEAN,
            std: IMAGENET_STD,
            post: Post::SigmoidIfLogits,
        },
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageQuality {
    pub image: String,
    /// Intersection over union of alpha >= 0.5.
    pub iou: f64,
    /// Mean absolute alpha difference over the whole image (0..1).
    pub mae: f64,
    /// Mean absolute alpha difference within BAND px of the reference contour.
    pub edge_mae: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutoutRun {
    pub model: String,
    pub size: u32,
    pub device: Device,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub load_ms: f64,
    pub first_run_ms: f64,
    pub mean_ms: f64,
    pub min_ms: f64,
    pub runs: usize,
    pub foreground_fraction: f64,
    pub quality: Vec<ImageQuality>,
    pub error: Option<String>,
}

fn preprocess(img: &RgbaImage, s: &Spec) -> Array4<f32> {
    let small = image::imageops::resize(img, s.size, s.size, FilterType::Triangle);
    let n = s.size as usize;
    let mut t = Array4::<f32>::zeros((1, 3, n, n));
    for (x, y, p) in small.enumerate_pixels() {
        for c in 0..3 {
            t[[0, c, y as usize, x as usize]] = (f32::from(p[c]) / 255.0 - s.mean[c]) / s.std[c];
        }
    }
    t
}

fn infer(
    session: &mut ort::session::Session,
    input_name: &str,
    t: &Array4<f32>,
    post: Post,
) -> Result<Vec<f32>, String> {
    let input = Tensor::from_array(t.clone()).map_err(err)?;
    let out = session
        .run(ort::inputs![input_name => input])
        .map_err(err)?;
    // The first output is the final mask for every model here (U2-Net's d0, IS-Net's first side output).
    let raw: Vec<f32> = out[0]
        .try_extract_array::<f32>()
        .map_err(err)?
        .iter()
        .copied()
        .collect();
    Ok(match post {
        Post::SigmoidIfLogits if raw.iter().any(|v| *v < -0.01 || *v > 1.01) => {
            raw.iter().map(|v| 1.0 / (1.0 + (-v).exp())).collect()
        }
        Post::SigmoidIfLogits => raw,
        Post::MinMax => {
            let (lo, hi) = raw
                .iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
            let span = (hi - lo).max(1e-6);
            raw.iter().map(|v| (v - lo) / span).collect()
        }
    })
}

/// The comparison grid for an image: longest side EVAL_SIDE, aspect kept.
fn eval_dims(img: &RgbaImage) -> (u32, u32) {
    let scale = EVAL_SIDE as f64 / img.width().max(img.height()) as f64;
    (
        ((img.width() as f64 * scale).round() as u32).max(1),
        ((img.height() as f64 * scale).round() as u32).max(1),
    )
}

/// A square model mask resized onto the image's comparison grid.
fn to_eval(mask: &[f32], side: u32, dims: (u32, u32)) -> GrayImage {
    let m = GrayImage::from_fn(side, side, |x, y| {
        Luma([(mask[(y * side + x) as usize].clamp(0.0, 1.0) * 255.0).round() as u8])
    });
    image::imageops::resize(&m, dims.0, dims.1, FilterType::Triangle)
}

/// Pixels within BAND of the reference's 0.5 contour (separable square dilation of the edge map).
fn contour_band(r: &GrayImage) -> Vec<bool> {
    let (w, h) = (r.width() as usize, r.height() as usize);
    let fg = |x: usize, y: usize| r.get_pixel(x as u32, y as u32)[0] >= 128;
    let mut edge = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let f = fg(x, y);
            edge[y * w + x] = (x + 1 < w && fg(x + 1, y) != f) || (y + 1 < h && fg(x, y + 1) != f);
        }
    }
    let mut horiz = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(BAND), (x + BAND).min(w - 1));
            horiz[y * w + x] = (a..=b).any(|i| edge[y * w + i]);
        }
    }
    let mut band = vec![false; w * h];
    for y in 0..h {
        let (a, b) = (y.saturating_sub(BAND), (y + BAND).min(h - 1));
        for x in 0..w {
            band[y * w + x] = (a..=b).any(|j| horiz[j * w + x]);
        }
    }
    band
}

fn compare(name: &str, m: &GrayImage, r: &GrayImage) -> ImageQuality {
    let (mut inter, mut union, mut sum, mut band_sum, mut band_n) = (0u64, 0u64, 0f64, 0f64, 0u64);
    let band = contour_band(r);
    for (i, (a, b)) in m.pixels().zip(r.pixels()).enumerate() {
        let (a, b) = (a[0] as f64 / 255.0, b[0] as f64 / 255.0);
        let d = (a - b).abs();
        sum += d;
        if band[i] {
            band_sum += d;
            band_n += 1;
        }
        let (fa, fb) = (a >= 0.5, b >= 0.5);
        inter += u64::from(fa && fb);
        union += u64::from(fa || fb);
    }
    let n = (m.width() * m.height()) as f64;
    ImageQuality {
        image: name.to_owned(),
        iou: if union == 0 {
            1.0
        } else {
            inter as f64 / union as f64
        },
        mae: sum / n,
        edge_mae: band_sum / band_n.max(1) as f64,
    }
}

pub struct Inputs<'a> {
    pub names: &'a [String],
    pub images: &'a [RgbaImage],
    /// Where this run's masks go (one PNG per image, on the comparison grid).
    pub mask_dir: PathBuf,
    /// The reference run's mask directory, when it exists and is not this run.
    pub ref_dir: Option<PathBuf>,
}

/// Benchmarks one model at one size on one device. `passes` timed passes over
/// all images follow one warm-up inference.
pub fn run(
    env: &Environment,
    model: &Path,
    model_id: &str,
    s: Spec,
    device: Device,
    inp: &Inputs,
    passes: usize,
) -> CutoutRun {
    let mut r = CutoutRun {
        model: model_id.to_owned(),
        size: s.size,
        device,
        inputs: vec![],
        outputs: vec![],
        load_ms: 0.0,
        first_run_ms: 0.0,
        mean_ms: 0.0,
        min_ms: 0.0,
        runs: 0,
        foreground_fraction: 0.0,
        quality: vec![],
        error: None,
    };
    let result = (|| -> Result<(), String> {
        let t0 = Instant::now();
        let mut session = ortx::session(env, model, device, 8)?;
        r.load_ms = t0.elapsed().as_secs_f64() * 1000.0;
        (r.inputs, r.outputs) = ortx::signature(&session);
        let input_name = session.inputs()[0].name().to_owned();
        let tensors: Vec<Array4<f32>> = inp.images.iter().map(|i| preprocess(i, &s)).collect();

        let t1 = Instant::now();
        infer(&mut session, &input_name, &tensors[0], s.post)?;
        r.first_run_ms = t1.elapsed().as_secs_f64() * 1000.0;

        std::fs::create_dir_all(&inp.mask_dir).map_err(err)?;
        let mut times = Vec::new();
        let (mut fg, mut px) = (0usize, 0usize);
        for pass in 0..passes {
            for (i, t) in tensors.iter().enumerate() {
                let t_run = Instant::now();
                let mask = infer(&mut session, &input_name, t, s.post)?;
                times.push(t_run.elapsed().as_secs_f64() * 1000.0);
                if pass > 0 {
                    continue;
                }
                fg += mask.iter().filter(|v| **v > 0.5).count();
                px += mask.len();
                let m = to_eval(&mask, s.size, eval_dims(&inp.images[i]));
                m.save(inp.mask_dir.join(format!("{}.png", inp.names[i])))
                    .map_err(err)?;
                if let Some(ref_dir) = &inp.ref_dir {
                    if let Ok(reference) =
                        image::open(ref_dir.join(format!("{}.png", inp.names[i])))
                    {
                        let reference = reference.to_luma8();
                        if reference.dimensions() == m.dimensions() {
                            r.quality.push(compare(&inp.names[i], &m, &reference));
                        }
                    }
                }
            }
        }
        r.runs = times.len();
        r.mean_ms = times.iter().sum::<f64>() / times.len().max(1) as f64;
        r.min_ms = times.iter().copied().fold(f64::INFINITY, f64::min);
        r.foreground_fraction = fg as f64 / px.max(1) as f64;
        Ok(())
    })();
    r.error = result.err();
    r
}
