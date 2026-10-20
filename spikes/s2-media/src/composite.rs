//! Export-side GPU composite: NV12 -> wgpu (shared WGSL effects) -> NV12.
//!
//! Synchronous per frame (submit, then wait for the readback). That is the
//! simplest correct shape and a lower bound on what a pipelined exporter does.

use ffmpeg::format::Pixel;
use ffmpeg::util::frame;
use ffmpeg_next as ffmpeg;
use wgpu::util::DeviceExt;

const SHADER: &str = concat!(
    include_str!("../../s1-preview/src/shaders/effects.wgsl"),
    "\n",
    include_str!("composite.wgsl")
);

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Draw {
    rect: [f32; 4],
    adjust: [f32; 4],
    blur_step: [f32; 2],
    pad: [f32; 2],
}

const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
fn padded(row: u32) -> u32 {
    row.div_ceil(ALIGN) * ALIGN
}

pub struct Compositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    w: u32,
    h: u32,
    in_y: wgpu::Texture,
    in_uv: wgpu::Texture,
    rgba: wgpu::Texture,
    out_y: wgpu::Texture,
    out_uv: wgpu::Texture,
    read_y: wgpu::Buffer,
    read_uv: wgpu::Buffer,
    groups: [wgpu::BindGroup; 3],
    pipes: [wgpu::RenderPipeline; 3],
    pub frames: u64,
    pub gpu_wait_s: f64,
}

impl Compositor {
    pub fn new(w: u32, h: u32) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .expect("adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("device");
        let tex = |label, width, height, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        use wgpu::TextureFormat as F;
        use wgpu::TextureUsages as U;
        let in_y = tex("in_y", w, h, F::R8Unorm, U::TEXTURE_BINDING | U::COPY_DST);
        let in_uv = tex(
            "in_uv",
            w / 2,
            h / 2,
            F::Rg8Unorm,
            U::TEXTURE_BINDING | U::COPY_DST,
        );
        let rgba = tex(
            "rgba",
            w,
            h,
            F::Rgba8Unorm,
            U::TEXTURE_BINDING | U::RENDER_ATTACHMENT,
        );
        let out_y = tex(
            "out_y",
            w,
            h,
            F::R8Unorm,
            U::RENDER_ATTACHMENT | U::COPY_SRC,
        );
        let out_uv = tex(
            "out_uv",
            w / 2,
            h / 2,
            F::Rg8Unorm,
            U::RENDER_ATTACHMENT | U::COPY_SRC,
        );
        let buffer = |size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        };
        let read_y = buffer(u64::from(padded(w) * h));
        let read_uv = buffer(u64::from(padded(w) * (h / 2)));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipe = |entry: &str, format: F| {
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
                    targets: &[Some(format.into())],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipes = [
            pipe("fs_nv12_fx", F::Rgba8Unorm),
            pipe("fs_to_y", F::R8Unorm),
            pipe("fs_to_uv", F::Rg8Unorm),
        ];
        // Same adjust values as the S1 preview's FX_ADJUST.
        let draw = Draw {
            rect: [0.0, 0.0, 1.0, 1.0],
            adjust: [0.05, 1.15, 1.3, 1.0],
            blur_step: [0.0; 2],
            pad: [0.0; 2],
        };
        let ubo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&draw),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let (vy, vuv, vrgba) = (
            in_y.create_view(&Default::default()),
            in_uv.create_view(&Default::default()),
            rgba.create_view(&Default::default()),
        );
        let group = |i: usize, extra: Vec<wgpu::BindGroupEntry>| {
            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubo.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ];
            entries.extend(extra);
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipes[i].get_bind_group_layout(0),
                entries: &entries,
            })
        };
        let groups = [
            group(
                0,
                vec![
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&vy),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&vuv),
                    },
                ],
            ),
            group(
                1,
                vec![wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&vrgba),
                }],
            ),
            group(
                2,
                vec![wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&vrgba),
                }],
            ),
        ];
        Self {
            device,
            queue,
            w,
            h,
            in_y,
            in_uv,
            rgba,
            out_y,
            out_uv,
            read_y,
            read_uv,
            groups,
            pipes,
            frames: 0,
            gpu_wait_s: 0.0,
        }
    }

    /// NV12 in, NV12 out (same size), through the shared colour adjust.
    pub fn process(&mut self, src: &frame::Video) -> Result<frame::Video, String> {
        if src.format() != Pixel::NV12 {
            return Err(format!(
                "composite takes NV12, got {:?} (10-bit composite not in the spike)",
                src.format()
            ));
        }
        let (w, h) = (self.w, self.h);
        let write = |tex: &wgpu::Texture, data: &[u8], stride: usize, tw: u32, th: u32| {
            self.queue.write_texture(
                tex.as_image_copy(),
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride as u32),
                    rows_per_image: Some(th),
                },
                wgpu::Extent3d {
                    width: tw,
                    height: th,
                    depth_or_array_layers: 1,
                },
            );
        };
        write(&self.in_y, src.data(0), src.stride(0), w, h);
        write(&self.in_uv, src.data(1), src.stride(1), w / 2, h / 2);

        let mut enc = self.device.create_command_encoder(&Default::default());
        let targets = [
            self.rgba.create_view(&Default::default()),
            self.out_y.create_view(&Default::default()),
            self.out_uv.create_view(&Default::default()),
        ];
        for i in 0..3 {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets[i],
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
            pass.set_pipeline(&self.pipes[i]);
            pass.set_bind_group(0, &self.groups[i], &[]);
            pass.draw(0..6, 0..1);
        }
        let copy = |enc: &mut wgpu::CommandEncoder,
                    tex: &wgpu::Texture,
                    buf: &wgpu::Buffer,
                    tw: u32,
                    th: u32,
                    bpp: u32| {
            enc.copy_texture_to_buffer(
                tex.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded(tw * bpp)),
                        rows_per_image: Some(th),
                    },
                },
                wgpu::Extent3d {
                    width: tw,
                    height: th,
                    depth_or_array_layers: 1,
                },
            );
        };
        copy(&mut enc, &self.out_y, &self.read_y, w, h, 1);
        copy(&mut enc, &self.out_uv, &self.read_uv, w / 2, h / 2, 2);
        self.queue.submit([enc.finish()]);

        let t0 = std::time::Instant::now();
        self.read_y.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.read_uv
            .slice(..)
            .map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        self.gpu_wait_s += t0.elapsed().as_secs_f64();

        let mut dst = frame::Video::new(Pixel::NV12, w, h);
        {
            let y = self
                .read_y
                .slice(..)
                .get_mapped_range()
                .map_err(|e| e.to_string())?;
            let uv = self
                .read_uv
                .slice(..)
                .get_mapped_range()
                .map_err(|e| e.to_string())?;
            let (ys, uvs) = (dst.stride(0), dst.stride(1));
            let (py, puv) = (padded(w) as usize, padded(w) as usize);
            for row in 0..h as usize {
                dst.data_mut(0)[row * ys..row * ys + w as usize]
                    .copy_from_slice(&y[row * py..row * py + w as usize]);
            }
            for row in 0..(h / 2) as usize {
                dst.data_mut(1)[row * uvs..row * uvs + w as usize]
                    .copy_from_slice(&uv[row * puv..row * puv + w as usize]);
            }
        }
        self.read_y.unmap();
        self.read_uv.unmap();
        dst.set_pts(src.pts());
        self.frames += 1;
        Ok(dst)
    }
}
