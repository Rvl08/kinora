// Typed wrappers for the spike's Tauri commands and the packet wire format
// (see src-tauri/src/demux.rs).

import { invoke } from '@tauri-apps/api/core';

export interface SourceInfo {
  id: number;
  path: string;
  codec: string | null;
  codecName: string;
  codedWidth: number;
  codedHeight: number;
  description: number[] | null;
  durationUs: number;
  frameRate: number;
  frameCount: number;
  colorSpace: string;
  colorRange: string;
  hasAudio: boolean;
}

export interface Clock {
  mediaUs: number;
  playing: boolean;
  device: boolean;
  latencyUs: number;
  underruns: number;
  rate: number;
}

export interface Packet {
  key: boolean;
  ptsUs: number;
  durUs: number;
  data: Uint8Array;
}

const FLAG_KEY = 1;
const FLAG_EOS = 2;
const HEADER = 21;

export function parsePackets(buf: ArrayBuffer): { packets: Packet[]; eos: boolean } {
  const view = new DataView(buf);
  const packets: Packet[] = [];
  let eos = false;
  let o = 0;
  while (o + HEADER <= buf.byteLength) {
    const len = view.getUint32(o, true);
    const flags = view.getUint8(o + 4);
    const ptsUs = Number(view.getBigInt64(o + 5, true));
    const durUs = Number(view.getBigInt64(o + 13, true));
    o += HEADER;
    if (flags & FLAG_EOS) {
      eos = true;
      continue;
    }
    packets.push({ key: (flags & FLAG_KEY) !== 0, ptsUs, durUs, data: new Uint8Array(buf, o, len) });
    o += len;
  }
  return { packets, eos };
}

export const ipc = {
  getConfig: () => invoke<Record<string, unknown>>('get_config'),
  openSource: (path: string) => invoke<SourceInfo>('open_source', { path }),
  closeSource: (id: number) => invoke<void>('close_source', { id }),
  readPackets: (id: number, maxBytes: number, maxPackets: number) =>
    invoke<ArrayBuffer>('read_packets', { id, maxBytes, maxPackets }),
  seekSource: (id: number, tUs: number) => invoke<void>('seek_source', { id, tUs: Math.round(tUs) }),
  audioLoad: (path: string) => invoke<void>('audio_load', { path }),
  audioPlay: () => invoke<void>('audio_play'),
  audioPause: () => invoke<void>('audio_pause'),
  audioSeek: (tUs: number) => invoke<void>('audio_seek', { tUs: Math.round(tUs) }),
  clock: () => invoke<Clock>('clock'),
  systemStats: () => invoke<Record<string, number>>('system_stats'),
  parityCompare: (rgba: Uint8Array, path: string, tUs: number, variant: string) =>
    invoke<Record<string, unknown>>('parity_compare', rgba, {
      headers: { 'x-path': encodeURIComponent(path), 'x-t-us': String(Math.round(tUs)), 'x-variant': variant },
    }),
  writeReport: (path: string, report: unknown) => invoke<void>('write_report', { path, report }),
  exitApp: () => invoke<void>('exit_app'),
};
