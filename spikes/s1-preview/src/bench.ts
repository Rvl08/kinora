// Scenario runner: measures what ADR-0003's acceptance criteria name and
// writes one JSON report.

import type { Compositor } from './compositor';
import { ipc } from './ipc';
import { nextFrame, Player } from './player';

export interface Scenario {
  name: string;
  files: string[];
  hw?: HardwareAcceleration;
  effects?: boolean;
  text?: boolean;
  playSeconds: number;
  seeks: number;
}

export interface ParitySpec {
  file: string;
  tUs: number[];
  variants: ('plain' | 'fx')[];
}

export interface BenchConfig {
  mode: 'bench';
  mediaDir: string;
  report: string;
  runs: Scenario[];
  parity?: ParitySpec;
  exitWhenDone?: boolean;
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function pct(values: number[], p: number): number | null {
  if (values.length === 0) return null;
  const s = [...values].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.max(0, Math.ceil((p / 100) * s.length) - 1))];
}
/** Max without spreading (a 10-minute run has ~150k samples: too many arguments). */
const maxOf = (v: number[]) => v.reduce((m, x) => (x > m ? x : m), 0);
const mean = (v: number[]) => (v.length ? v.reduce((a, b) => a + b, 0) / v.length : null);
const round = (v: number | null, d = 2) => (v === null ? null : Number(v.toFixed(d)));

/** Deterministic PRNG so every run seeks to the same places. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Least-squares slope of media time against wall time (1.0 = no drift). */
function clockRate(samples: { wallMs: number; mediaUs: number }[]): number | null {
  if (samples.length < 10) return null;
  const xs = samples.map((s) => s.wallMs);
  const ys = samples.map((s) => s.mediaUs / 1000);
  const mx = mean(xs)!;
  const my = mean(ys)!;
  let num = 0;
  let den = 0;
  for (let i = 0; i < xs.length; i++) {
    num += (xs[i] - mx) * (ys[i] - my);
    den += (xs[i] - mx) ** 2;
  }
  return den === 0 ? null : num / den;
}

async function runScenario(comp: Compositor, cfg: BenchConfig, s: Scenario, log: (m: string) => void) {
  const files = s.files.map((f) => (/^[a-zA-Z]:|^\//.test(f) ? f : `${cfg.mediaDir}\\${f}`));
  const hw = s.hw ?? 'prefer-hardware';
  const base = { name: s.name, files: s.files, hw, effects: !!s.effects, text: !!s.text, layers: files.length };
  log(`▶ ${s.name}`);
  const tOpen = performance.now();
  const opened = await Player.open(comp, { files, hw, effects: !!s.effects, text: !!s.text });
  if ('unsupported' in opened) {
    return {
      ...base,
      supported: false,
      codecs: opened.infos.map((i) => ({ codecName: i.codecName, codec: i.codec })),
      note: 'WebCodecs cannot decode this source; production plays it through a proxy (ADR-0003 §5).',
    };
  }
  const player = opened.player;
  const info = player.layers[0].info;
  player.start();

  // Time to first frame on every layer.
  while (!player.layers.every((l) => l.current)) {
    await nextFrame();
    if (performance.now() - tOpen > 15_000) break;
  }
  const firstFrameMs = performance.now() - tOpen;

  // Playback.
  const stats: Record<string, number>[] = [];
  let sampling = true;
  const sampler = (async () => {
    await ipc.systemStats(); // first sample primes CPU accounting
    while (sampling) {
      await sleep(2000);
      if (sampling) stats.push(await ipc.systemStats());
    }
  })();
  player.resetMetrics();
  player.recording = true;
  const wall0 = performance.now();
  await ipc.audioPlay();
  await sleep(s.playSeconds * 1000);
  await ipc.audioPause();
  const playedMs = performance.now() - wall0;
  player.recording = false;
  sampling = false;
  await sampler;

  const m = player.metrics;
  const intervals = m.intervalsMs.filter((v) => v > 0);
  const renderFps = intervals.length ? 1000 / mean(intervals)! : null;
  const p99Interval = pct(intervals, 99);
  const frameUs = player.layers[0].frameUs;
  const rate = clockRate(m.clockSamples);
  const perLayer = player.layers.map((l) => {
    const st = l.stats;
    const total = st.presented + st.skipped;
    return {
      codec: l.info.codec,
      decoded: st.decoded,
      presented: st.presented,
      skipped: st.skipped,
      skipPct: total ? round((100 * st.skipped) / total, 3) : null,
      contentFps: round(st.presented / (playedMs / 1000), 2),
      starvedTicks: st.starvedTicks,
      decodeErrors: st.decodeErrors,
      seekBackoffs: st.seekBackoffs,
      lastError: l.lastError,
      ipc: {
        calls: st.ipcCalls,
        mbPerSec: round(st.ipcBytes / 1048576 / (playedMs / 1000), 2),
        msP50: round(pct(st.ipcMs, 50)),
        msP95: round(pct(st.ipcMs, 95)),
      },
    };
  });
  const skew = m.skewUs;
  const play = {
    seconds: round(playedMs / 1000, 2),
    renderFpsMean: round(renderFps),
    renderFpsP1Low: p99Interval ? round(1000 / p99Interval) : null,
    perLayer,
    avSkewMs: { p50: round((pct(skew, 50) ?? 0) / 1000), p95: round((pct(skew, 95) ?? 0) / 1000), max: round(maxOf(skew) / 1000) },
    // Late: the frame on screen outlived its slot because its successor was not decoded in time.
    lateFraction: m.ticks ? round(m.lateTicks / m.ticks, 4) : null,
    nominalFrameMs: round(frameUs / 1000),
    clock: {
      device: player.clock.device,
      outputLatencyMs: round(player.clock.latencyUs / 1000),
      underruns: player.clock.underruns,
      mediaPerWallRate: rate === null ? null : Number(rate.toFixed(6)),
      driftMsPerMinute: rate === null ? null : round((rate - 1) * 60_000, 3),
    },
  };

  // Seeks: random targets, all layers must show the exact covering frame.
  const rand = mulberry32(0x5eed + s.name.length);
  const dur = info.durationUs;
  const seekMs: number[] = [];
  let accurate = 0;
  for (let k = 0; k < s.seeks; k++) {
    const target = Math.round(1_000_000 + rand() * Math.max(0, dur - 3_000_000));
    const t0 = performance.now();
    await player.seek(target);
    while (!player.ready(target) && performance.now() - t0 < 5000) await nextFrame();
    await nextFrame(); // shown
    const ms = performance.now() - t0;
    seekMs.push(ms);
    // Exact: the frame on screen starts at or before the target and the next
    // decoded frame (if any) starts after it.
    if (player.layers.every((l) => l.current && l.current.timestamp <= target && (l.nextTimestamp() ?? Infinity) > target)) accurate++;
    player.release();
    await sleep(50);
  }
  const seeks = {
    count: s.seeks,
    latencyMs: { p50: round(pct(seekMs, 50)), p95: round(pct(seekMs, 95)), max: round(maxOf(seekMs)) },
    frameAccurate: accurate,
  };
  const cores = stats[0]?.logicalCpus ?? 1;
  const system = {
    samples: stats.length,
    hostCpuPctOneCore: round(mean(stats.map((x) => x.hostCpu))),
    webviewCpuPctOneCore: round(mean(stats.map((x) => x.webviewCpu))),
    machineCpuPct: round((mean(stats.map((x) => x.hostCpu + x.webviewCpu)) ?? 0) / cores),
    hostMemMbMax: round(maxOf(stats.map((x) => x.hostMemMb)), 1),
    webviewMemMbMax: round(maxOf(stats.map((x) => x.webviewMemMb)), 1),
    webviewProcesses: stats.at(-1)?.webviewProcesses ?? null,
  };
  await player.close();
  log(`  done: ${play.renderFpsMean} fps, seek p95 ${seeks.latencyMs.p95} ms`);
  return {
    ...base,
    supported: true,
    source: {
      codec: info.codec,
      codecName: info.codecName,
      width: info.codedWidth,
      height: info.codedHeight,
      frameRate: round(info.frameRate, 3),
      colorSpace: info.colorSpace,
      colorRange: info.colorRange,
    },
    timeToFirstFrameMs: round(firstFrameMs),
    play,
    seeks,
    system,
  };
}

async function runParity(comp: Compositor, cfg: BenchConfig, spec: ParitySpec, log: (m: string) => void) {
  const file = /^[a-zA-Z]:/.test(spec.file) ? spec.file : `${cfg.mediaDir}\\${spec.file}`;
  const opened = await Player.open(comp, { files: [file], hw: 'prefer-hardware', effects: false, text: false });
  if ('unsupported' in opened) return [{ error: 'unsupported source' }];
  const p = opened.player;
  p.start();
  const results = [];
  for (const t of spec.tUs) {
    await p.seek(t);
    const t0 = performance.now();
    while (!p.ready(t) && performance.now() - t0 < 5000) await nextFrame();
    await nextFrame();
    const frame = p.layers[0].current;
    if (!frame) {
      results.push({ tUs: t, error: 'no frame' });
      continue;
    }
    for (const v of spec.variants) {
      const rgba = await comp.renderParity(frame, v);
      try {
        const r = await ipc.parityCompare(rgba, file, t, v);
        results.push({ ...r, browserFrameUs: frame.timestamp });
        log(`  parity t=${t / 1e6}s ${v}: ${Number(r.psnrDb).toFixed(2)} dB`);
      } catch (e) {
        results.push({ tUs: t, variant: v, error: String(e) });
      }
    }
  }
  await p.close();
  return results;
}

/** Pass/fail against ADR-0003's acceptance criteria, where the run can judge them. */
function evaluate(runs: Record<string, any>[], parity: Record<string, any>[]) {
  const find = (name: string) => runs.find((r) => r.name === name && r.supported);
  const out: Record<string, unknown>[] = [];
  const r1 = find('1080p30-h264-3layers-fx-text');
  if (r1) {
    const skip = Math.max(...r1.play.perLayer.map((l: any) => l.skipPct ?? 100));
    out.push({
      criterion: '1080p30 H.264, 3 layers + 2 effects: >= 30 fps, frame drop < 1%',
      measured: `${r1.play.renderFpsMean} fps render, content ${r1.play.perLayer.map((l: any) => l.contentFps).join('/')} fps, max skip ${skip}%`,
      pass: r1.play.perLayer.every((l: any) => l.contentFps >= 29) && skip < 1,
      hardware: 'discrete GPU only (no iGPU available)',
    });
  }
  const r2 = find('2160p60-hevc10-1layer');
  if (r2) {
    out.push({
      criterion: '4K60 HEVC 10-bit on a discrete GPU: >= 50 fps',
      measured: `${r2.play.perLayer[0].contentFps} fps content`,
      pass: r2.play.perLayer[0].contentFps >= 50,
    });
  }
  const drift = runs.find((r) => r.name.startsWith('drift') && r.supported);
  if (drift) {
    out.push({
      criterion: 'A/V offset <= 1 frame after 10 minutes',
      measured: `skew p95 ${drift.play.avSkewMs.p95} ms, late ${drift.play.lateFraction}, clock drift ${drift.play.clock.driftMsPerMinute} ms/min vs wall`,
      pass: drift.play.lateFraction !== null && drift.play.lateFraction < 0.01,
    });
  }
  const seeks = runs.filter((r) => r.supported && r.seeks?.count);
  if (seeks.length) {
    out.push({
      criterion: 'Random seek <= 150 ms (p95)',
      measured: seeks.map((r) => `${r.name}: ${r.seeks.latencyMs.p95} ms (${r.seeks.frameAccurate}/${r.seeks.count} exact)`).join('; '),
      pass: seeks.every((r) => r.seeks.latencyMs.p95 <= 150),
    });
  }
  const ok = parity.filter((p) => typeof p.psnrDb === 'number');
  if (ok.length) {
    out.push({
      criterion: 'Preview vs export PSNR >= 40 dB',
      measured: ok.map((p) => `${p.variant}@${p.tUs / 1e6}s ${Number(p.psnrDb).toFixed(2)} dB`).join('; '),
      pass: ok.every((p) => p.psnrDb >= 40),
    });
  }
  return out;
}

export async function runBench(comp: Compositor, cfg: BenchConfig, log: (m: string) => void): Promise<void> {
  const started = new Date().toISOString();
  const runs: Record<string, unknown>[] = [];
  for (const s of cfg.runs) {
    try {
      runs.push(await runScenario(comp, cfg, s, log));
    } catch (e) {
      runs.push({ name: s.name, error: String(e) });
      log(`  error: ${e}`);
    }
  }
  const parity = cfg.parity ? await runParity(comp, cfg, cfg.parity, log) : [];
  const refresh = await estimateRefresh();
  const report = {
    spike: 'S1 preview pipeline',
    started,
    finished: new Date().toISOString(),
    environment: {
      userAgent: navigator.userAgent,
      adapter: comp.adapterInfo,
      canvasFormat: comp.format,
      devicePixelRatio: window.devicePixelRatio,
      screen: `${screen.width}x${screen.height}`,
      refreshHzEstimate: refresh,
    },
    evaluation: evaluate(runs as Record<string, any>[], parity as Record<string, any>[]),
    runs,
    parity,
  };
  await ipc.writeReport(cfg.report, report);
  log(`report written: ${cfg.report}`);
  if (cfg.exitWhenDone !== false) await ipc.exitApp();
}

async function estimateRefresh(): Promise<number> {
  const ts: number[] = [];
  for (let i = 0; i < 121; i++) ts.push(await nextFrame());
  const d = ts.slice(1).map((t, i) => t - ts[i]);
  return round(1000 / pct(d, 50)!, 1) ?? 0;
}
