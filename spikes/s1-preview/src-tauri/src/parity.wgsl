// wgpu "exporter" entry points. Prepended with ../../src/shaders/effects.wgsl.

@group(0) @binding(0) var<uniform> draw: Draw;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(3) var tex: texture_2d<f32>;
@group(0) @binding(4) var tex_y: texture_2d<f32>;
@group(0) @binding(5) var tex_u: texture_2d<f32>;
@group(0) @binding(6) var tex_v: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
  return quad_vertex(i, draw.rect);
}

// Decoded video frame as planes; this runtime does YUV -> RGB itself.
@fragment
fn fs_yuv(in: VsOut) -> @location(0) vec4f {
  let y = textureSampleLevel(tex_y, samp, in.uv, 0.0).r;
  let u = textureSampleLevel(tex_u, samp, in.uv, 0.0).r;
  let v = textureSampleLevel(tex_v, samp, in.uv, 0.0).r;
  return vec4f(color_adjust(yuv709_limited(y, u, v), draw.adjust), 1.0);
}

@fragment
fn fs_blur(in: VsOut) -> @location(0) vec4f {
  return vec4f(blur9(tex, samp, in.uv, draw.blur_step).rgb, 1.0);
}
