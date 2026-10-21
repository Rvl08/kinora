// Shared helpers for the code signing scripts (ADR-0007): find signtool,
// recognise PE files, read Authenticode signatures, match the policy.

import { spawnSync } from 'node:child_process';
import { existsSync, openSync, readSync, closeSync, readdirSync, statSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const root = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');

export async function loadPolicy(path = join(root, 'scripts', 'signing', 'policy.json')) {
  return JSON.parse(await readFile(path, 'utf8'));
}

/** True when the file starts with an MZ header that points at a PE signature. */
export function isPe(path) {
  let fd;
  try {
    fd = openSync(path, 'r');
    const head = Buffer.alloc(64);
    if (readSync(fd, head, 0, 64, 0) < 64 || head.toString('latin1', 0, 2) !== 'MZ') return false;
    const peOffset = head.readUInt32LE(0x3c);
    const sig = Buffer.alloc(4);
    if (readSync(fd, sig, 0, 4, peOffset) < 4) return false;
    return sig.equals(Buffer.from('PE\0\0', 'latin1'));
  } catch {
    return false;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
}

/** Every PE file under the given files/directories (by content, not extension). */
export function collectPe(paths) {
  const out = [];
  const walk = (p) => {
    const s = statSync(p);
    if (s.isDirectory()) {
      for (const e of readdirSync(p)) walk(join(p, e));
    } else if (isPe(p)) {
      out.push(resolve(p));
    }
  };
  for (const p of paths) walk(p);
  return out;
}

/** signtool.exe from KINORA_SIGNTOOL, PATH, or the newest x64 Windows SDK. */
export function findSigntool() {
  if (process.env.KINORA_SIGNTOOL && existsSync(process.env.KINORA_SIGNTOOL))
    return process.env.KINORA_SIGNTOOL;
  const where = spawnSync('where.exe', ['signtool.exe'], { encoding: 'utf8' });
  if (where.status === 0) return where.stdout.split(/\r?\n/)[0].trim();
  const kits = join(
    process.env['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)',
    'Windows Kits',
    '10',
    'bin',
  );
  if (!existsSync(kits)) return null;
  const versions = readdirSync(kits)
    .filter(
      (v) => /^\d+\.\d+\.\d+\.\d+$/.test(v) && existsSync(join(kits, v, 'x64', 'signtool.exe')),
    )
    .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  return versions.length ? join(kits, versions.at(-1), 'x64', 'signtool.exe') : null;
}

// Reads Authenticode signatures for many files in one PowerShell process.
const READ_SIGNATURES = `
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$paths = [Console]::In.ReadToEnd() | ConvertFrom-Json
$out = foreach ($p in $paths) {
  $s = Get-AuthenticodeSignature -LiteralPath $p
  $c = $s.SignerCertificate
  [ordered]@{
    path = $p
    status = [string]$s.Status
    message = $s.StatusMessage
    signer = if ($c) { $c.Subject } else { $null }
    issuer = if ($c) { $c.Issuer } else { $null }
    thumbprint = if ($c) { $c.Thumbprint } else { $null }
    notAfter = if ($c) { $c.NotAfter.ToString('o') } else { $null }
    keyAlgorithm = if ($c) { $c.PublicKey.Oid.FriendlyName } else { $null }
    keySize = if ($c -and $c.PublicKey.Key) { $c.PublicKey.Key.KeySize } else { $null }
    timestamper = if ($s.TimeStamperCertificate) { $s.TimeStamperCertificate.Subject } else { $null }
  }
}
ConvertTo-Json -InputObject @($out) -Depth 3 -Compress
`;

export function readSignatures(paths) {
  if (paths.length === 0) return [];
  const r = spawnSync(
    'powershell.exe',
    ['-NoProfile', '-NonInteractive', '-Command', READ_SIGNATURES],
    {
      input: JSON.stringify(paths),
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  if (r.status !== 0) throw new Error(`Get-AuthenticodeSignature failed: ${r.stderr}`);
  return JSON.parse(r.stdout);
}

/** File digest algorithms of the signatures on a file, from `signtool verify /v /all`. */
export function digestAlgorithms(signtool, path) {
  const r = spawnSync(signtool, ['verify', '/pa', '/all', '/v', path], { encoding: 'utf8' });
  return [...r.stdout.matchAll(/Hash of file \((\w+)\)/g)].map((m) => m[1].toLowerCase());
}

function globToRegex(glob) {
  return new RegExp(
    `^${glob
      .replace(/[.+^${}()|[\]\\]/g, '\\$&')
      .replace(/\*/g, '.*')
      .replace(/\?/g, '.')}$`,
    'i',
  );
}

/** The third-party rule whose file pattern matches, if any. */
export function thirdPartyRule(policy, path) {
  const name = basename(path);
  return policy.thirdParty.find((r) => r.files.some((g) => globToRegex(g).test(name))) ?? null;
}

/** Whether a signer subject is one of ours (exact match on the subject's CN part or the full subject). */
export function isOurs(policy, signer, { allowTest = false } = {}) {
  if (!signer) return false;
  const names = [...policy.publishers, ...(allowTest ? policy.testPublishers : [])];
  return names.some((n) => signer === n || signer.startsWith(`${n},`));
}
