// WebGPU compositor: decoded VideoFrames come in as external textures
// (zero-copy, YUV->RGB by the browser) and go through the shared WGSL effects.

import effectsWgsl from './shaders/effects.wgsl?raw';
import browserWgsl from './shaders/browser.wgsl?raw';

export interface Adjust {
  brightness: number;
  contrast: number;
  saturation: number;
  enabled: boolean;
}

export interface LayerDraw {
  frame: VideoFrame;
  rect: [number, number, number, number];
  adjust?: Adjust;
  blur?: boolean;
}

/** Same values as `draws()` in src-tauri/src/parity.rs. */
export const FX_ADJUST: Adjust = { brightness: 0.05, contrast: 1.15, saturation: 1.3, enabled: true };
const NO_ADJUST: Adjust = { brightness: 0, contrast: 1, saturation: 1, enabled: false };

const W = 1920;
const H = 1080;
const TEXT_H = 160;
const DRAW_BYTES = 48;
const MAX_DRAWS = 32;

export class Compositor {
  readonly adapterInfo: Record<string, string>;
  private readonly sampler: GPUSampler;
  private readonly module: GPUShaderModule;
  private readonly pipelines = new Map<string, GPURenderPipeline>();
  private readonly uniforms: GPUBuffer[];
  private readonly scratch: GPUTexture[];
  private readonly text: GPUTexture;
  private readonly textCanvas = new OffscreenCanvas(W, TEXT_H);
  private textValue = '';
  private drawIndex = 0;

  private constructor(
    readonly device: GPUDevice,
    private readonly context: GPUCanvasContext,
    readonly format: GPUTextureFormat,
    info: GPUAdapterInfo,
  ) {
    this.adapterInfo = {
      vendor: info.vendor,
      architecture: info.architecture,
      device: info.device,
      description: info.description,
    };
    this.sampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear' });
    this.module = device.createShaderModule({ code: `${effectsWgsl}\n${browserWgsl}` });
    this.uniforms = Array.from({ length: MAX_DRAWS }, () =>
      device.createBuffer({ size: DRAW_BYTES, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST }),
    );
    const usage =
      GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC;
    this.scratch = [0, 1, 2].map(() =>
      device.createTexture({ size: [W, H], format: 'rgba8unorm', usage }),
    );
    this.text = device.createTexture({
      size: [W, TEXT_H],
      format: 'rgba8unorm',
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
    });
  }

  static async create(canvas: HTMLCanvasElement): Promise<Compositor> {
    if (!navigator.gpu) throw new Error('WebGPU is not available in this WebView');
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
    if (!adapter) throw new Error('No WebGPU adapter');
    const device = await adapter.requestDevice();
    device.lost.then((l) => console.error('GPU device lost', l.message));
    const context = canvas.getContext('webgpu');
    if (!context) throw new Error('No webgpu canvas context');
    const format = navigator.gpu.getPreferredCanvasFormat();
    context.configure({ device, format, alphaMode: 'opaque' });
    return new Compositor(device, context, format, adapter.info);
  }

  private pipeline(entry: string, format: GPUTextureFormat, blend = false): GPURenderPipeline {
    const key = `${entry}|${format}|${blend}`;
    let p = this.pipelines.get(key);
    if (!p) {
      p = this.device.createRenderPipeline({
        layout: 'auto',
        vertex: { module: this.module, entryPoint: 'vs' },
        fragment: {
          module: this.module,
          entryPoint: entry,
          targets: [
            {
              format,
              blend: blend
                ? {
                    color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
                    alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
                  }
                : undefined,
            },
          ],
        },
      });
      this.pipelines.set(key, p);
    }
    return p;
  }

  private uniform(rect: number[], adjust: Adjust, step: [number, number]): GPUBuffer {
    const buf = this.uniforms[this.drawIndex++ % MAX_DRAWS];
    const a = new Float32Array([
      ...rect,
      adjust.brightness,
      adjust.contrast,
      adjust.saturation,
      adjust.enabled ? 1 : 0,
      step[0],
      step[1],
      0,
      0,
    ]);
    this.device.queue.writeBuffer(buf, 0, a);
    return buf;
  }

  private pass(
    enc: GPUCommandEncoder,
    target: GPUTextureView,
    format: GPUTextureFormat,
    entry: string,
    rect: number[],
    source: { ext?: GPUExternalTexture; tex?: GPUTexture },
    opts: { adjust?: Adjust; step?: [number, number]; clear: boolean; blend?: boolean },
  ): void {
    const pipe = this.pipeline(entry, format, opts.blend);
    const entries: GPUBindGroupEntry[] = [
      { binding: 0, resource: { buffer: this.uniform(rect, opts.adjust ?? NO_ADJUST, opts.step ?? [0, 0]) } },
      { binding: 1, resource: this.sampler },
    ];
    if (source.ext) entries.push({ binding: 2, resource: source.ext });
    if (source.tex) entries.push({ binding: 3, resource: source.tex.createView() });
    const group = this.device.createBindGroup({ layout: pipe.getBindGroupLayout(0), entries });
    const pass = enc.beginRenderPass({
      colorAttachments: [
        {
          view: target,
          loadOp: opts.clear ? 'clear' : 'load',
          storeOp: 'store',
          clearValue: { r: 0, g: 0, b: 0, a: 1 },
        },
      ],
    });
    pass.setPipeline(pipe);
    pass.setBindGroup(0, group);
    pass.draw(6);
    pass.end();
  }

  /** Background (optional adjust + blur) into `target`; returns nothing. */
  private background(
    enc: GPUCommandEncoder,
    target: GPUTextureView,
    format: GPUTextureFormat,
    d: LayerDraw,
  ): void {
    const ext = this.device.importExternalTexture({ source: d.frame });
    if (d.blur) {
      const [a, b] = this.scratch;
      this.pass(enc, a.createView(), 'rgba8unorm', 'fs_video', [0, 0, 1, 1], { ext }, { adjust: d.adjust, clear: true });
      this.pass(enc, b.createView(), 'rgba8unorm', 'fs_blur', [0, 0, 1, 1], { tex: a }, { step: [1 / W, 0], clear: true });
      this.pass(enc, target, format, 'fs_blur', d.rect, { tex: b }, { step: [0, 1 / H], clear: true });
    } else {
      this.pass(enc, target, format, 'fs_video', d.rect, { ext }, { adjust: d.adjust, clear: true });
    }
  }

  setText(value: string): void {
    if (value === this.textValue) return;
    this.textValue = value;
    const g = this.textCanvas.getContext('2d')!;
    g.clearRect(0, 0, W, TEXT_H);
    g.fillStyle = 'rgba(0,0,0,0.55)';
    g.beginPath();
    g.roundRect(48, 32, W - 96, TEXT_H - 64, 24);
    g.fill();
    g.fillStyle = '#fff';
    g.font = '600 56px Segoe UI, sans-serif';
    g.textBaseline = 'middle';
    g.fillText(value, 88, TEXT_H / 2);
    this.device.queue.copyExternalImageToTexture(
      { source: this.textCanvas },
      { texture: this.text, premultipliedAlpha: true },
      [W, TEXT_H],
    );
  }

  render(layers: (LayerDraw | null)[], withText: boolean): void {
    const enc = this.device.createCommandEncoder();
    const view = this.context.getCurrentTexture().createView();
    const [bg, ...rest] = layers;
    if (bg) {
      this.background(enc, view, this.format, bg);
    } else {
      const pass = enc.beginRenderPass({
        colorAttachments: [{ view, loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
      });
      pass.end();
    }
    for (const d of rest) {
      if (!d) continue;
      const ext = this.device.importExternalTexture({ source: d.frame });
      this.pass(enc, view, this.format, 'fs_video', d.rect, { ext }, { adjust: d.adjust, clear: false });
    }
    if (withText) {
      this.pass(enc, view, this.format, 'fs_overlay', [0, 1 - TEXT_H / H - 0.02, 1, TEXT_H / H], { tex: this.text }, {
        clear: false,
        blend: true,
      });
    }
    this.device.queue.submit([enc.finish()]);
  }

  /** Renders one frame full-size into rgba8unorm and reads it back. */
  async renderParity(frame: VideoFrame, variant: 'plain' | 'fx'): Promise<Uint8Array> {
    const enc = this.device.createCommandEncoder();
    const target = this.scratch[2];
    this.background(enc, target.createView(), 'rgba8unorm', {
      frame,
      rect: [0, 0, 1, 1],
      adjust: variant === 'fx' ? FX_ADJUST : NO_ADJUST,
      blur: variant === 'fx',
    });
    const bytesPerRow = W * 4;
    const buf = this.device.createBuffer({ size: bytesPerRow * H, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    enc.copyTextureToBuffer({ texture: target }, { buffer: buf, bytesPerRow, rowsPerImage: H }, [W, H]);
    this.device.queue.submit([enc.finish()]);
    await buf.mapAsync(GPUMapMode.READ);
    const out = new Uint8Array(buf.getMappedRange().slice(0));
    buf.unmap();
    buf.destroy();
    return out;
  }
}
