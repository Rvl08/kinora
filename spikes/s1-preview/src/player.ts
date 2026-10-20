// Layers + master clock + render loop.
//
// The audio clock lives in Rust. Fetching it inside requestAnimationFrame
// would put an IPC round trip before every draw, so the clock is sampled
// asynchronously and extrapolated with performance.now() at draw time.

import { FX_ADJUST, type Compositor, type LayerDraw } from './compositor';
import { ipc, type Clock, type SourceInfo } from './ipc';
import { Layer } from './layer';

export interface PlayerOptions {
  files: string[];
  hw: HardwareAcceleration;
  effects: boolean;
  text: boolean;
}

const RECTS: [number, number, number, number][] = [
  [0, 0, 1, 1],
  [0.62, 0.06, 0.34, 0.34],
  [0.62, 0.44, 0.34, 0.34],
];

export interface TickMetrics {
  intervalsMs: number[];
  skewUs: number[];
  ticks: number;
  lateTicks: number;
  clockSamples: { wallMs: number; mediaUs: number }[];
}

export class Player {
  readonly metrics: TickMetrics = { intervalsMs: [], skewUs: [], ticks: 0, lateTicks: 0, clockSamples: [] };
  clock: Clock = { mediaUs: 0, playing: false, device: false, latencyUs: 0, underruns: 0, rate: 0 };
  private clockAt = performance.now();
  private clockFetching = false;
  private raf = 0;
  private lastTick = 0;
  recording = false;
  /** When set, the render uses this time instead of the clock (seeks). */
  holdUs: number | null = null;

  private constructor(
    readonly layers: Layer[],
    private readonly comp: Compositor,
    private readonly opts: PlayerOptions,
  ) {}

  static async open(
    comp: Compositor,
    opts: PlayerOptions,
  ): Promise<{ player: Player } | { unsupported: SourceInfo[]; infos: SourceInfo[] }> {
    const infos: SourceInfo[] = [];
    for (const f of opts.files) infos.push(await ipc.openSource(f));
    const pixels = infos[0].codedWidth * infos[0].codedHeight;
    const layers = infos.map((i) => new Layer(i, opts.hw, pixels > 2_500_000 ? 4 : 8));
    const unsupported: SourceInfo[] = [];
    for (const l of layers) if (!(await l.supported())) unsupported.push(l.info);
    if (unsupported.length > 0) {
      for (const i of infos) await ipc.closeSource(i.id);
      return { unsupported, infos };
    }
    await ipc.audioLoad(infos[0].path);
    for (const l of layers) l.start();
    return { player: new Player(layers, comp, opts) };
  }

  private refreshClock(): void {
    if (this.clockFetching) return;
    this.clockFetching = true;
    ipc
      .clock()
      .then((c) => {
        this.clock = c;
        this.clockAt = performance.now();
        if (this.recording && c.playing) this.metrics.clockSamples.push({ wallMs: this.clockAt, mediaUs: c.mediaUs });
      })
      .finally(() => {
        this.clockFetching = false;
      });
  }

  nowUs(): number {
    if (this.holdUs !== null) return this.holdUs;
    const c = this.clock;
    return c.playing ? c.mediaUs + (performance.now() - this.clockAt) * 1000 : c.mediaUs;
  }

  start(): void {
    const loop = (ts: number) => {
      this.raf = requestAnimationFrame(loop);
      this.tick(ts);
    };
    this.raf = requestAnimationFrame(loop);
  }

  stop(): void {
    cancelAnimationFrame(this.raf);
  }

  private tick(ts: number): void {
    this.refreshClock();
    if (this.recording && this.lastTick > 0) this.metrics.intervalsMs.push(ts - this.lastTick);
    this.lastTick = ts;
    const t = this.nowUs();
    const playing = this.clock.playing && this.holdUs === null;
    const draws: (LayerDraw | null)[] = this.layers.map((l, i) => {
      const frame = l.pick(t, playing);
      if (!frame) return null;
      return {
        frame,
        rect: RECTS[i] ?? RECTS[RECTS.length - 1],
        adjust: this.opts.effects ? FX_ADJUST : undefined,
        blur: this.opts.effects && i === 0,
      };
    });
    if (this.opts.text) {
      const s = (t / 1e6).toFixed(1);
      this.comp.setText(`Kinora S1 · ${s} s · ${this.layers.length} layer · ${this.layers[0].info.codecName}`);
    }
    this.comp.render(draws, this.opts.text);
    for (const l of this.layers) l.markShown();
    const cur = this.layers[0].current;
    if (this.recording && playing && cur) {
      this.metrics.skewUs.push(t - cur.timestamp);
      this.metrics.ticks++;
      // Late: the frame on screen has outlived its slot and its successor is not
      // decoded yet (pick() always advances to the newest due frame otherwise).
      const l0 = this.layers[0];
      if (l0.nextTimestamp() === null && !l0.eos && cur.timestamp + (cur.duration ?? l0.frameUs) < t) this.metrics.lateTicks++;
    }
  }

  resetMetrics(): void {
    this.metrics.intervalsMs = [];
    this.metrics.skewUs = [];
    this.metrics.ticks = 0;
    this.metrics.lateTicks = 0;
    this.metrics.clockSamples = [];
    this.lastTick = 0;
    for (const l of this.layers) {
      Object.assign(l.stats, {
        decoded: 0,
        presented: 0,
        skipped: 0,
        starvedTicks: 0,
        decodeErrors: 0,
        seekBackoffs: 0,
        ipcCalls: 0,
        ipcBytes: 0,
        ipcMs: [],
      });
    }
  }

  async seek(tUs: number): Promise<void> {
    this.holdUs = tUs;
    await ipc.audioSeek(tUs);
    await Promise.all(this.layers.map((l) => l.seek(tUs)));
  }

  ready(tUs: number): boolean {
    return this.layers.every((l) => l.readyFor(tUs));
  }

  release(): void {
    this.holdUs = null;
  }

  async close(): Promise<void> {
    this.stop();
    await ipc.audioPause();
    await Promise.all(this.layers.map((l) => l.close()));
  }
}

export const nextFrame = () => new Promise<number>((r) => requestAnimationFrame(r));
