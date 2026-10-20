// Generates the synthetic Spike S1 test media with the repository's LGPL
// FFmpeg (vendor/ffmpeg-lgpl). Every frame carries its frame number and
// timestamp, so a preview can be checked for frame accuracy by eye or by
// readback.
//
// Usage: node spikes/s1-preview/scripts/make-media.mjs [--force]
// Output: tests/media/s1/*.mp4|mov (git-ignored)

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const ffmpeg = join(root, 'vendor', 'ffmpeg-lgpl', 'bin', 'ffmpeg.exe');
const out = join(root, 'tests', 'media', 's1');
const force = process.argv.includes('--force');
mkdirSync(out, { recursive: true });

const font = 'C\\:/Windows/Fonts/consola.ttf';
const label = (size) =>
  `drawtext=fontfile='${font}':fontsize=${size}:fontcolor=white:box=1:boxcolor=black@0.6:` +
  `x=(w-text_w)/2:y=h*0.42:text='%{frame_num}  %{pts\\:hms}'`;

// Audio: a 1 kHz beep for 100 ms at the start of every second, so A/V offset
// can be judged against the frame counter.
const beep = (seconds) =>
  `sine=frequency=1000:sample_rate=48000:duration=${seconds},` +
  `volume='if(lt(mod(t,1),0.1),0.5,0)':eval=frame`;

const fixtures = [
  {
    name: '1080p30-h264.mp4',
    video: 'testsrc2=size=1920x1080:rate=30',
    seconds: 60,
    filters: label(96),
    codec: ['-c:v', 'h264_nvenc', '-preset', 'p5', '-b:v', '12M', '-g', '60', '-pix_fmt', 'yuv420p'],
  },
  {
    name: '2160p60-hevc10.mp4',
    video: 'testsrc2=size=3840x2160:rate=60',
    seconds: 60,
    filters: `${label(192)},format=p010le`,
    codec: ['-c:v', 'hevc_nvenc', '-profile:v', 'main10', '-preset', 'p5', '-b:v', '40M', '-g', '120', '-tag:v', 'hvc1'],
  },
  {
    // Variable frame rate, like a phone: drop roughly one frame in seven.
    name: '1080p-vfr-h264.mp4',
    video: 'testsrc2=size=1920x1080:rate=30',
    seconds: 60,
    filters: `${label(96)},select='not(eq(mod(n\\,7)\\,3))'`,
    codec: ['-c:v', 'h264_nvenc', '-preset', 'p5', '-b:v', '12M', '-g', '60', '-pix_fmt', 'yuv420p', '-fps_mode', 'vfr'],
  },
  {
    name: '1080p30-av1.mp4',
    video: 'testsrc2=size=1920x1080:rate=30',
    seconds: 30,
    filters: label(96),
    codec: ['-c:v', 'av1_nvenc', '-preset', 'p5', '-b:v', '8M', '-g', '60', '-pix_fmt', 'yuv420p'],
  },
  {
    // Proxy-like: short GOP (10 frames), the shape production proxies take for scrubbing.
    name: '1080p30-h264-gop10.mp4',
    video: 'testsrc2=size=1920x1080:rate=30',
    seconds: 60,
    filters: label(96),
    codec: ['-c:v', 'h264_nvenc', '-preset', 'p5', '-b:v', '16M', '-g', '10', '-bf', '0', '-pix_fmt', 'yuv420p'],
  },
  {
    // Long clip for the 10-minute A/V drift run.
    name: '1080p30-h264-11min.mp4',
    video: 'testsrc2=size=1920x1080:rate=30',
    seconds: 660,
    filters: label(96),
    codec: ['-c:v', 'h264_nvenc', '-preset', 'p4', '-b:v', '5M', '-g', '60', '-pix_fmt', 'yuv420p'],
  },
  {
    // Not decodable by WebCodecs: exercises the proxy path.
    name: '2160p30-prores.mov',
    video: 'testsrc2=size=3840x2160:rate=30',
    seconds: 10,
    filters: `${label(192)},format=yuv422p10le`,
    codec: ['-c:v', 'prores_ks', '-profile:v', '2'],
  },
];

for (const f of fixtures) {
  const target = join(out, f.name);
  if (existsSync(target) && !force) {
    console.log(`keep ${f.name}`);
    continue;
  }
  const args = [
    '-hide_banner', '-loglevel', 'error', '-y',
    '-f', 'lavfi', '-i', `${f.video}:duration=${f.seconds}`,
    '-f', 'lavfi', '-i', beep(f.seconds),
    // Frame-level tags: NVENC writes the VUI from the frames, not from -colorspace.
    '-vf', `${f.filters},setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv`,
    ...f.codec,
    // Explicit colour metadata so the browser and the Rust renderer agree on
    // the YUV -> RGB matrix (the parity check depends on it).
    '-colorspace', 'bt709', '-color_primaries', 'bt709', '-color_trc', 'bt709', '-color_range', 'tv',
    '-c:a', 'aac', '-b:a', '160k',
    '-movflags', '+faststart',
    '-shortest', target,
  ];
  console.log(`make ${f.name}`);
  const t0 = Date.now();
  const r = spawnSync(ffmpeg, args, { stdio: 'inherit' });
  if (r.status !== 0) {
    console.error(`FAILED ${f.name} (exit ${r.status})`);
    process.exit(1);
  }
  console.log(`  done in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
}
