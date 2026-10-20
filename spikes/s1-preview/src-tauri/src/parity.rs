//! Preview/export parity (ADR-0003 acceptance: PSNR >= 40 dB).
//!
//! The WebView renders frame `t` through WebGPU (browser YUV->RGB via
//! `importExternalTexture`, shared WGSL effects) and sends the RGBA pixels.
//! Here the same frame is decoded in software by FFmpeg, uploaded as Y/U/V
//! planes and rendered by wgpu with the same shared WGSL, then compared.

use ffmpeg::{Rational, Rescale, format, frame, media};
use ffmpeg_next as ffmpeg;
use serde_json::{Value, json};
use wgpu::util::DeviceExt;

const SHADER: &str = concat!(
    include_str!("../../src/shaders/effects.wgsl"),
    "\n",
    include_str!("parity.wgsl")
);

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Draw {
    rect: [f32; 4],
    adjust: [f32; 4],
    blur_step: [f32; 2],
    pad: [f32; 2],
}

/// Must match `parityDraws` in src/parity.ts.
fn draws(variant: &str, w: u32, h: u32) -> Result<Vec<Draw>, String> {
    let full = [0.0, 0.0, 1.0, 1.0];
    let off = [0.0; 4];
    let fx = [0.05, 1.15, 1.3, 1.0];
    let step_h = [1.0 / w as f32, 0.0];
    let step_v = [0.0, 1.0 / h as f32];
    match variant {
        "plain" => Ok(vec![Draw {
            rect: full,
            adjust: off,
            blur_step: [0.0; 2],
            pad: [0.0; 2],
        }]),
        "fx" => Ok(vec![
            Draw {
                rect: full,
                adjust: fx,
                blur_step: [0.0; 2],
                pad: [0.0; 2],
            },
            Draw {
                rect: full,
                adjust: off,
                blur_step: step_h,
                pad: [0.0; 2],
            },
            Draw {
                rect: full,
                adjust: off,
                blur_step: step_v,
                pad: [0.0; 2],
            },
        ]),
        other => Err(format!("unknown parity variant {other}")),
    }
}

struct Planes {
    w: u32,
    h: u32,
    pts_us: i64,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

/// Software-decodes the frame displayed at `t_us` (the one whose interval covers it).
fn decode_frame(path: &str, t_us: i64) -> Result<Planes, String> {
    let mut input = format::input(path).map_err(|e| e.to_string())?;
    let stream = input.streams().best(media::Type::Video).ok_or("no video")?;
    let index = stream.index();
    let tb = stream.time_base();
    let ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .map_err(|e| e.to_string())?;
    let mut decoder = ctx.decoder().video().map_err(|e| e.to_string())?;
    if decoder.format() != format::Pixel::YUV420P {
        return Err(format!(
            "parity supports 8-bit yuv420p only, got {:?}",
            decoder.format()
        ));
    }
    input.seek(t_us, ..t_us).map_err(|e| e.to_string())?;
    let us = Rational(1, 1_000_000);
    let mut best: Option<(i64, frame::Video)> = None;
    let mut decoded = frame::Video::empty();
    let take = |decoded: &frame::Video, best: &mut Option<(i64, frame::Video)>| -> bool {
        let pts = decoded.pts().unwrap_or(0).rescale(tb, us);
        if pts <= t_us {
            *best = Some((pts, decoded.clone()));
            false
        } else {
            true // passed the target: the previous frame is the one on screen
        }
    };
    'outer: for (s, packet) in input.packets() {
        if s.index() != index {
            continue;
        }
        decoder.send_packet(&packet).map_err(|e| e.to_string())?;
        while decoder.receive_frame(&mut decoded).is_ok() {
            if take(&decoded, &mut best) {
                break 'outer;
            }
        }
    }
    let (pts_us, f) = best.ok_or("no frame at or before t")?;
    let (w, h) = (f.width(), f.height());
    let plane = |i: usize, pw: u32, ph: u32| -> Vec<u8> {
        let stride = f.stride(i);
        let data = f.data(i);
        (0..ph as usize)
            .flat_map(|row| {
                data[row * stride..row * stride + pw as usize]
                    .iter()
                    .copied()
            })
            .collect()
    };
    Ok(Planes {
        w,
        h,
        pts_us,
        y: plane(0, w, h),
        u: plane(1, w / 2, h / 2),
        v: plane(2, w / 2, h / 2),
    })
}

fn render(planes: &Planes, variant: &str) -> Result<Vec<u8>, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|e| e.to_string())?;
    let (w, h) = (planes.w, planes.h);

    let plane_tex = |data: &[u8], pw: u32, ph: u32| {
        device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: pw,
                    height: ph,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            data,
        )
    };
    let ty = plane_tex(&planes.y, w, h);
    let tu = plane_tex(&planes.u, w / 2, h / 2);
    let tv = plane_tex(&planes.v, w / 2, h / 2);
    let target = |label| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    };
    let targets = [target("a"), target("b"), target("c")];
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("parity"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = |entry: &str| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    };
    let yuv = pipeline("fs_yuv");
    let blur = pipeline("fs_blur");

    let passes = draws(variant, w, h)?;
    let mut encoder = device.create_command_encoder(&Default::default());
    let views: Vec<_> = targets
        .iter()
        .map(|t| t.create_view(&Default::default()))
        .collect();
    let (vy, vu, vv) = (
        ty.create_view(&Default::default()),
        tu.create_view(&Default::default()),
        tv.create_view(&Default::default()),
    );
    // plain: yuv -> c.  fx: yuv -> a, blur a -> b, blur b -> c.
    let order: &[(usize, Option<usize>)] = if passes.len() == 1 {
        &[(2, None)]
    } else {
        &[(0, None), (1, Some(0)), (2, Some(1))]
    };
    for (draw, &(dst, src)) in passes.iter().zip(order) {
        let ubo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(draw),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let (pipe, entries): (&wgpu::RenderPipeline, Vec<wgpu::BindGroupEntry>) = match src {
            None => (
                &yuv,
                vec![
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ubo.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&vy),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&vu),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(&vv),
                    },
                ],
            ),
            Some(s) => (
                &blur,
                vec![
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ubo.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&views[s]),
                    },
                ],
            ),
        };
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipe.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &views[dst],
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..6, 0..1);
    }
    let row = w * 4; // 1920 * 4 = 7680, a multiple of 256
    if row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT != 0 {
        return Err(format!(
            "width {w} needs row padding (not implemented in the spike)"
        ));
    }
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        targets[2].as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| e.to_string())?;
    let pixels = slice
        .get_mapped_range()
        .map_err(|e| e.to_string())?
        .to_vec();
    Ok(pixels)
}

pub fn compare(path: &str, t_us: i64, variant: &str, browser: &[u8]) -> Result<Value, String> {
    let planes = decode_frame(path, t_us)?;
    let native = render(&planes, variant)?;
    if native.len() != browser.len() {
        return Err(format!(
            "size mismatch: native {} bytes, browser {}",
            native.len(),
            browser.len()
        ));
    }
    let mut sq = [0f64; 3];
    let mut max = [0u8; 3];
    let mut over8 = 0usize;
    for (a, b) in native.chunks_exact(4).zip(browser.chunks_exact(4)) {
        for c in 0..3 {
            let d = a[c].abs_diff(b[c]);
            sq[c] += f64::from(d) * f64::from(d);
            max[c] = max[c].max(d);
            if d > 8 {
                over8 += 1;
            }
        }
    }
    let n = (native.len() / 4) as f64;
    let psnr = |mse: f64| {
        if mse == 0.0 {
            f64::INFINITY
        } else {
            10.0 * (255.0f64 * 255.0 / mse).log10()
        }
    };
    let mse_rgb = (sq[0] + sq[1] + sq[2]) / (3.0 * n);
    Ok(json!({
        "variant": variant,
        "tUs": t_us,
        "referencePtsUs": planes.pts_us,
        "width": planes.w,
        "height": planes.h,
        "psnrDb": psnr(mse_rgb),
        "psnrPerChannelDb": [psnr(sq[0] / n), psnr(sq[1] / n), psnr(sq[2] / n)],
        "maxAbsDiff": max,
        "fractionOver8": over8 as f64 / (3.0 * n),
    }))
}
