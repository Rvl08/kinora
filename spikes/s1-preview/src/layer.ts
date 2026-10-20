// One timeline layer: a source demuxed in Rust, decoded here by WebCodecs.
//
// Packets are pulled (backpressure): a read is issued only while the decoder
// queue and the decoded-frame queue have room. Frames are presented by
// `pick(t)`, which keeps the frame whose interval covers the clock time.

import { ipc, parsePackets, type SourceInfo } from './ipc';

export interface LayerStats {
  decoded: number;
  presented: number;
  skipped: number;
  starvedTicks: number;
  decodeErrors: number;
  seekBackoffs: number;
  ipcCalls: number;
  ipcBytes: number;
  ipcMs: number[];
}

export class Layer {
  readonly stats: LayerStats = {
    decoded: 0,
    presented: 0,
    skipped: 0,
    starvedTicks: 0,
    decodeErrors: 0,
    seekBackoffs: 0,
    ipcCalls: 0,
    ipcBytes: 0,
    ipcMs: [],
  };
  current: VideoFrame | null = null;
  private currentShown = false;
  private frames: VideoFrame[] = [];
  private decoder: VideoDecoder | null = null;
  private generation = 0;
  private inflight: Promise<void> | null = null;
  private inputDone = false;
  private needKey = true;
  private dropBeforeUs = -Infinity;
  /** True from a seek until the frame covering its target is decoded. */
  private seeking = false;
  /** During a seek: the latest frame at or before the target, held until a
   *  later frame (or the end of the stream) proves it is the one on screen. */
  private candidate: VideoFrame | null = null;
  private backoffUs = 0;
  private closed = false;
  eos = false;
  lastError: string | null = null;
  readonly frameUs: number;

  constructor(
    readonly info: SourceInfo,
    readonly hw: HardwareAcceleration,
    private readonly maxFrames: number,
  ) {
    this.frameUs = info.frameRate > 0 ? 1e6 / info.frameRate : 33_333;
  }

  config(): VideoDecoderConfig {
    const cfg: VideoDecoderConfig = {
      codec: this.info.codec ?? 'unsupported',
      codedWidth: this.info.codedWidth,
      codedHeight: this.info.codedHeight,
      hardwareAcceleration: this.hw,
    };
    if (this.info.description) cfg.description = new Uint8Array(this.info.description);
    return cfg;
  }

  async supported(): Promise<boolean> {
    if (!this.info.codec) return false;
    const r = await VideoDecoder.isConfigSupported(this.config());
    return r.supported === true;
  }

  start(): void {
    this.decoder = new VideoDecoder({
      output: (frame) => this.onFrame(frame),
      error: (e) => {
        this.stats.decodeErrors++;
        this.lastError = String(e);
      },
    });
    this.decoder.configure(this.config());
    this.decoder.addEventListener('dequeue', () => this.pump());
    this.pump();
  }

  private onFrame(frame: VideoFrame): void {
    if (this.closed) {
      frame.close();
      return;
    }
    // Frames arrive in presentation order. Before a seek target, keep only the
    // latest one: with variable frame rate a frame's duration cannot say whether
    // it is still on screen at the target, only the next frame's timestamp can.
    if (frame.timestamp <= this.dropBeforeUs) {
      this.candidate?.close();
      this.candidate = frame;
      return;
    }
    if (this.seeking && !this.candidate && frame.timestamp > this.dropBeforeUs) {
      // The demuxer landed on a key frame after the target (imprecise index):
      // nothing at or before the target will ever arrive. Seek again, earlier.
      frame.close();
      this.stats.seekBackoffs++;
      const target = this.dropBeforeUs;
      this.backoffUs += 2_000_000;
      void this.restartAt(target, Math.max(0, target - this.backoffUs));
      return;
    }
    this.releaseCandidate();
    this.dropBeforeUs = -Infinity;
    this.seeking = false;
    this.backoffUs = 0;
    this.stats.decoded++;
    this.frames.push(frame);
  }

  private releaseCandidate(): void {
    if (this.candidate) {
      this.stats.decoded++;
      this.frames.push(this.candidate);
      this.candidate = null;
    }
  }

  /** Pulls packets while there is room downstream. Never runs twice at once. */
  pump(): void {
    if (this.inflight || this.closed || !this.decoder) return;
    this.inflight = this.pumpLoop().finally(() => {
      this.inflight = null;
    });
  }

  private async pumpLoop(): Promise<void> {
    const big = this.info.codedWidth * this.info.codedHeight > 2_500_000;
    while (
      !this.closed &&
      !this.inputDone &&
      this.decoder &&
      this.decoder.state === 'configured' &&
      // Playback keeps the decoder queue short (GPU memory); a seek fills it so the
      // hardware decoder never idles while it runs up to the target.
      this.decoder.decodeQueueSize < (this.seeking ? 32 : 3) &&
      this.frames.length < this.maxFrames
    ) {
      const generation = this.generation;
      const t0 = performance.now();
      const batch = this.seeking ? 32 : big ? 4 : 8;
      const buf = await ipc.readPackets(this.info.id, this.seeking ? 4_000_000 : big ? 1_500_000 : 600_000, batch);
      this.stats.ipcCalls++;
      this.stats.ipcBytes += buf.byteLength;
      this.stats.ipcMs.push(performance.now() - t0);
      if (generation !== this.generation || this.closed) return;
      const { packets, eos } = parsePackets(buf);
      for (const p of packets) {
        if (this.needKey && !p.key) continue;
        this.needKey = false;
        this.decoder.decode(
          new EncodedVideoChunk({
            type: p.key ? 'key' : 'delta',
            timestamp: p.ptsUs,
            duration: p.durUs > 0 ? p.durUs : undefined,
            data: p.data,
          }),
        );
      }
      if (eos) {
        this.inputDone = true;
        await this.decoder.flush().catch(() => undefined);
        this.releaseCandidate();
        this.seeking = false;
        this.eos = true;
      }
    }
  }

  /** Advances to the frame covering `tUs`; returns it (may be the same one). */
  pick(tUs: number, playing: boolean): VideoFrame | null {
    while (this.frames.length > 0 && this.frames[0].timestamp <= tUs) {
      const next = this.frames.shift()!;
      if (this.current) {
        if (!this.currentShown) this.stats.skipped++;
        this.current.close();
      }
      this.current = next;
      this.currentShown = false;
    }
    if (
      playing &&
      this.frames.length === 0 &&
      !this.eos &&
      this.current &&
      // Nothing decoded after the frame on screen, and it has run past its slot.
      this.current.timestamp + (this.current.duration ?? this.frameUs) < tUs
    ) {
      this.stats.starvedTicks++;
    }
    this.pump();
    return this.current;
  }

  markShown(): void {
    if (this.current && !this.currentShown) {
      this.currentShown = true;
      this.stats.presented++;
    }
  }

  /**
   * The frame on screen at `tUs` is the last one with timestamp <= tUs. It is
   * known once a later frame is decoded (or the stream ended).
   */
  readyFor(tUs: number): boolean {
    const all = [this.current, ...this.frames].filter((f): f is VideoFrame => !!f);
    const shown = all.filter((f) => f.timestamp <= tUs).at(-1);
    return !!shown && (all.some((f) => f.timestamp > tUs) || this.eos);
  }

  /** Timestamp of the next decoded frame after the current one, if any. */
  nextTimestamp(): number | null {
    return this.frames[0]?.timestamp ?? null;
  }

  /** Frame-accurate seek: key frame at or before `tUs`, then decode forward. */
  async seek(tUs: number): Promise<void> {
    this.backoffUs = 0;
    await this.restartAt(tUs, tUs);
  }

  /** Restarts decoding from the key frame at or before `fromUs`, showing `targetUs`. */
  private async restartAt(targetUs: number, fromUs: number): Promise<void> {
    this.generation++;
    if (this.inflight) await this.inflight;
    for (const f of this.frames) f.close();
    this.frames = [];
    this.candidate?.close();
    this.candidate = null;
    if (this.current) {
      this.current.close();
      this.current = null;
    }
    this.decoder?.reset();
    this.decoder?.configure(this.config());
    this.dropBeforeUs = targetUs;
    this.seeking = true;
    this.inputDone = false;
    this.eos = false;
    this.needKey = true;
    await ipc.seekSource(this.info.id, fromUs);
    this.pump();
  }

  async close(): Promise<void> {
    this.closed = true;
    this.generation++;
    if (this.inflight) await this.inflight.catch(() => undefined);
    for (const f of this.frames) f.close();
    this.frames = [];
    this.current?.close();
    this.current = null;
    this.candidate?.close();
    this.candidate = null;
    if (this.decoder && this.decoder.state !== 'closed') this.decoder.close();
    await ipc.closeSource(this.info.id);
  }
}
