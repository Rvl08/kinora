// Spike S1 shared effect library.
//
// Pure functions and types only. Each runtime adds its own bindings and
// entry points: browser.wgsl (WebGPU preview, texture_external input) and
// src-tauri/src/parity.wgsl (wgpu "exporter", Y/U/V plane input). Both
// include this file verbatim, so an effect is written once (ADR-0003).

struct Adjust {
  brightness: f32,
  contrast: f32,
  saturation: f32,
  enabled: f32,
}

// Per-draw uniform. `rect` is x, y, w, h in [0, 1] target space, y down.
struct Draw {
  rect: vec4f,
  adjust: Adjust,
  blur_step: vec2f,
  pad: vec2f,
}

struct VsOut {
  @builtin(position) pos: vec4f,
  @location(0) uv: vec2f,
}

fn quad_vertex(index: u32, rect: vec4f) -> VsOut {
  var corners = array<vec2f, 6>(
    vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
    vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
  );
  let c = corners[index];
  let p = vec2f(rect.x + c.x * rect.z, rect.y + c.y * rect.w);
  var out: VsOut;
  out.pos = vec4f(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
  out.uv = c;
  return out;
}

fn color_adjust(rgb: vec3f, a: Adjust) -> vec3f {
  if (a.enabled < 0.5) {
    return rgb;
  }
  var c = rgb + vec3f(a.brightness);
  c = (c - vec3f(0.5)) * a.contrast + vec3f(0.5);
  let luma = dot(c, vec3f(0.2126, 0.7152, 0.0722));
  c = mix(vec3f(luma), c, a.saturation);
  return clamp(c, vec3f(0.0), vec3f(1.0));
}

// 9-tap separable Gaussian; run once horizontally, once vertically.
fn blur9(tex: texture_2d<f32>, samp: sampler, uv: vec2f, step: vec2f) -> vec4f {
  var w = array<f32, 5>(0.2270270270, 0.1945945946, 0.1216216216, 0.0540540541, 0.0162162162);
  var acc = textureSampleLevel(tex, samp, uv, 0.0) * w[0];
  for (var i = 1; i < 5; i++) {
    let o = step * f32(i);
    acc += textureSampleLevel(tex, samp, uv + o, 0.0) * w[i];
    acc += textureSampleLevel(tex, samp, uv - o, 0.0) * w[i];
  }
  return acc;
}

// BT.709, limited range, 8-bit normalised samples -> gamma-encoded RGB.
fn yuv709_limited(y: f32, u: f32, v: f32) -> vec3f {
  let yy = (y * 255.0 - 16.0) / 219.0;
  let cb = (u * 255.0 - 128.0) / 224.0;
  let cr = (v * 255.0 - 128.0) / 224.0;
  return clamp(vec3f(
    yy + 1.5748 * cr,
    yy - 0.187324 * cb - 0.468124 * cr,
    yy + 1.8556 * cb,
  ), vec3f(0.0), vec3f(1.0));
}
