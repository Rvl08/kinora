import { runBench, type BenchConfig } from './bench';
import { Compositor } from './compositor';
import { ipc } from './ipc';
import { Player } from './player';

const hud = document.getElementById('hud')!;
const log = (m: string) => {
  console.log(m);
  hud.textContent = m;
};

async function interactive(comp: Compositor, mediaDir: string): Promise<void> {
  const file = `${mediaDir}\\1080p30-h264.mp4`;
  const opened = await Player.open(comp, {
    files: [file, file, file],
    hw: 'prefer-hardware',
    effects: true,
    text: true,
  });
  if ('unsupported' in opened) {
    log(`unsupported: ${opened.unsupported.map((i) => i.codecName).join(', ')}`);
    return;
  }
  const player = opened.player;
  player.start();
  const play = document.getElementById('play') as HTMLButtonElement;
  const seek = document.getElementById('seek') as HTMLInputElement;
  const duration = player.layers[0].info.durationUs;
  play.onclick = async () => {
    if (player.clock.playing) {
      await ipc.audioPause();
      play.textContent = 'Play';
    } else {
      player.release();
      await ipc.audioPlay();
      play.textContent = 'Pause';
    }
  };
  seek.oninput = async () => {
    await ipc.audioPause();
    play.textContent = 'Play';
    await player.seek((Number(seek.value) / 1000) * duration);
  };
  let last = performance.now();
  let frames = 0;
  const update = () => {
    frames++;
    const now = performance.now();
    if (now - last >= 500) {
      const fps = (frames * 1000) / (now - last);
      const s = player.layers.map((l) => `${l.stats.decoded}/${l.stats.presented}/${l.stats.skipped}`).join('  ');
      hud.textContent =
        `${fps.toFixed(0)} fps  t=${(player.nowUs() / 1e6).toFixed(3)} s  ` +
        `clock=${player.clock.device ? 'audio' : 'wall'} latency=${(player.clock.latencyUs / 1000).toFixed(1)} ms  ` +
        `decoded/shown/skipped: ${s}`;
      if (!seek.matches(':active')) seek.value = String(Math.round((player.nowUs() / duration) * 1000));
      frames = 0;
      last = now;
    }
    requestAnimationFrame(update);
  };
  requestAnimationFrame(update);
}

async function main(): Promise<void> {
  const cfg = (await ipc.getConfig()) as unknown as { mode: string; mediaDir: string };
  const comp = await Compositor.create(document.getElementById('view') as HTMLCanvasElement);
  if (cfg.mode === 'bench') {
    await runBench(comp, cfg as unknown as BenchConfig, log);
  } else {
    await interactive(comp, cfg.mediaDir);
  }
}

main().catch((e) => log(`fatal: ${e instanceof Error ? e.stack : String(e)}`));
