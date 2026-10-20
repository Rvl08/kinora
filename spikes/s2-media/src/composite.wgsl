// S2 export composite entry points. Prepended with the shared S1 library
// (spikes/s1-preview/src/shaders/effects.wgsl): same effect code as the preview.

@group(0) @binding(0) var<uniform> draw: Draw;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(3) var tex: texture_2d<f32>;
@group(0) @binding(4) var tex_y: texture_2d<f32>;
@group(0) @binding(5) var tex_uv: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
  return quad_vertex(i, draw.rect);
}

// NV12 in (Y + interleaved UV), RGB out, with the shared colour adjust.
@fragment
fn fs_nv12_fx(in: VsOut) -> @location(0) vec4f {
  let y = textureSampleLevel(tex_y, samp, in.uv, 0.0).r;
  let uv = textureSampleLevel(tex_uv, samp, in.uv, 0.0).rg;
  return vec4f(color_adjust(yuv709_limited(y, uv.x, uv.y), draw.adjust), 1.0);
}

fn luma709(c: vec3f) -> f32 {
  return dot(c, vec3f(0.2126, 0.7152, 0.0722));
}

// RGB -> Y' (BT.709, limited range), full resolution.
@fragment
fn fs_to_y(in: VsOut) -> @location(0) vec4f {
  let c = textureSampleLevel(tex, samp, in.uv, 0.0).rgb;
  return vec4f((16.0 + 219.0 * luma709(c)) / 255.0, 0.0, 0.0, 1.0);
}

// RGB -> Cb Cr (BT.709, limited range), half resolution: a bilinear sample at
// the shared corner of each 2x2 block averages the four pixels.
@fragment
fn fs_to_uv(in: VsOut) -> @location(0) vec4f {
  let c = textureSampleLevel(tex, samp, in.uv, 0.0).rgb;
  let y = luma709(c);
  let cb = (128.0 + 224.0 * (c.b - y) / 1.8556) / 255.0;
  let cr = (128.0 + 224.0 * (c.r - y) / 1.5748) / 255.0;
  return vec4f(cb, cr, 0.0, 1.0);
}
