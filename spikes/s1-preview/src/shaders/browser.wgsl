// WebGPU preview entry points. Prepended with effects.wgsl at load time.

@group(0) @binding(0) var<uniform> draw: Draw;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var ext: texture_external;
@group(0) @binding(3) var tex: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
  return quad_vertex(i, draw.rect);
}

// Decoded video frame (YUV -> RGB done by the browser) with colour adjust.
@fragment
fn fs_video(in: VsOut) -> @location(0) vec4f {
  let rgb = textureSampleBaseClampToEdge(ext, samp, in.uv).rgb;
  return vec4f(color_adjust(rgb, draw.adjust), 1.0);
}

@fragment
fn fs_blur(in: VsOut) -> @location(0) vec4f {
  return vec4f(blur9(tex, samp, in.uv, draw.blur_step).rgb, 1.0);
}

// Premultiplied overlay (titles).
@fragment
fn fs_overlay(in: VsOut) -> @location(0) vec4f {
  return textureSampleLevel(tex, samp, in.uv, 0.0);
}
