//! Background removal with BiRefNet-lite through ONNX Runtime (CPU / DirectML).

use std::path::Path;
use std::time::Instant;

use image::{GrayImage, Rgba, RgbaImage, imageops::FilterType};
use ndarray::Array4;
use ort::environment::Environment;
use ort::value::Tensor;
use serde::Serialize;

use crate::ortx::{self, Device, err};

const SIZE: u32 = 1024;
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutoutRun {
    pub model: String,
    pub device: Device,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub load_ms: f64,
    pub first_run_ms: f64,
    pub mean_ms: f64,
    pub min_ms: f64,
    pub runs: usize,
    pub foreground_fraction: f64,
    pub error: Option<String>,
}

fn preprocess(img: &RgbaImage) -> Array4<f32> {
    let small = image::imageops::resize(img, SIZE, SIZE, FilterType::Triangle);
    let mut t = Array4::<f32>::zeros((1, 3, SIZE as usize, SIZE as usize));
    for (x, y, p) in small.enumerate_pixels() {
        for c in 0..3 {
            t[[0, c, y as usize, x as usize]] = (f32::from(p[c]) / 255.0 - MEAN[c]) / STD[c];
        }
    }
    t
}

fn infer(
    session: &mut ort::session::Session,
    input_name: &str,
    t: &Array4<f32>,
) -> Result<Vec<f32>, String> {
    let input = Tensor::from_array(t.clone()).map_err(err)?;
    let out = session
        .run(ort::inputs![input_name => input])
        .map_err(err)?;
    // BiRefNet exports differ: some end in a sigmoid, some return logits.
    let raw: Vec<f32> = out[0]
        .try_extract_array::<f32>()
        .map_err(err)?
        .iter()
        .copied()
        .collect();
    let logits = raw.iter().any(|v| *v < -0.01 || *v > 1.01);
    Ok(if logits {
        raw.iter().map(|v| 1.0 / (1.0 + (-v).exp())).collect()
    } else {
        raw
    })
}

/// Benchmarks one model on one device; writes mask + cutout PNGs for the first image.
pub fn run(
    env: &Environment,
    model: &Path,
    device: Device,
    images: &[RgbaImage],
    runs: usize,
    out_prefix: Option<&Path>,
) -> CutoutRun {
    let name = model
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut r = CutoutRun {
        model: name,
        device,
        inputs: vec![],
        outputs: vec![],
        load_ms: 0.0,
        first_run_ms: 0.0,
        mean_ms: 0.0,
        min_ms: 0.0,
        runs,
        foreground_fraction: 0.0,
        error: None,
    };
    let result = (|| -> Result<(), String> {
        let t0 = Instant::now();
        let mut session = ortx::session(env, model, device, 8)?;
        r.load_ms = t0.elapsed().as_secs_f64() * 1000.0;
        (r.inputs, r.outputs) = ortx::signature(&session);
        let input_name = session.inputs()[0].name().to_owned();
        let tensors: Vec<Array4<f32>> = images.iter().map(preprocess).collect();

        let t1 = Instant::now();
        let mask = infer(&mut session, &input_name, &tensors[0])?;
        r.first_run_ms = t1.elapsed().as_secs_f64() * 1000.0;

        let mut times = Vec::new();
        for i in 0..runs {
            let t = Instant::now();
            infer(&mut session, &input_name, &tensors[i % tensors.len()])?;
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        r.mean_ms = times.iter().sum::<f64>() / times.len().max(1) as f64;
        r.min_ms = times.iter().copied().fold(f64::INFINITY, f64::min);
        r.foreground_fraction =
            mask.iter().filter(|v| **v > 0.5).count() as f64 / mask.len() as f64;

        if let Some(prefix) = out_prefix {
            let img = &images[0];
            let m = GrayImage::from_fn(SIZE, SIZE, |x, y| {
                image::Luma([(mask[(y * SIZE + x) as usize] * 255.0) as u8])
            });
            let m = image::imageops::resize(&m, img.width(), img.height(), FilterType::Triangle);
            m.save(prefix.with_extension("mask.png")).map_err(err)?;
            let cut = RgbaImage::from_fn(img.width(), img.height(), |x, y| {
                let p = img.get_pixel(x, y);
                Rgba([p[0], p[1], p[2], m.get_pixel(x, y)[0]])
            });
            cut.save(prefix.with_extension("cutout.png")).map_err(err)?;
        }
        Ok(())
    })();
    r.error = result.err();
    r
}
