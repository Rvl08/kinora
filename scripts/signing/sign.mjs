// Signs Windows PE files with Authenticode (ADR-0007). Provider-agnostic: the
// key can live in the Windows certificate store (smartcard, cloud HSM with a
// virtual card such as Certum SimplySign, OV token), in Azure Artifact
// Signing, or, for tests only, in a PFX file.
//
// Usage: node scripts/signing/sign.mjs [options] <file|dir>...
//   (Tauri's bundle.windows.signCommand passes one file as %1.)
//
// Options / environment:
//   --mode store|artifact-signing|pfx|dry-run   KINORA_SIGN_MODE (default: dry-run)
//   --thumbprint SHA1          KINORA_SIGN_THUMBPRINT   (store)
//   --dlib PATH --metadata PATH KINORA_SIGN_DLIB, KINORA_SIGN_METADATA (artifact-signing)
//   --pfx PATH                 KINORA_SIGN_PFX, password in KINORA_SIGN_PFX_PASSWORD (pfx, tests only)
//   --description TEXT         shown by Windows as the program name (default "Kinora")
//   --force                    re-sign files that already carry our signature
//   --allow-test               treat the test publisher as ours (self-tests)
//
// Never re-signs a file that carries a valid third-party signature allowed by
// policy.json (Microsoft, Cisco), and refuses to overwrite any other foreign
// signature. Every signature is SHA-256 with an RFC 3161 SHA-256 timestamp;
// timestamp servers are tried in order, so one server outage does not fail a release.

import { spawnSync } from 'node:child_process';
import { basename } from 'node:path';
import {
  collectPe,
  findSigntool,
  isOurs,
  loadPolicy,
  readSignatures,
  thirdPartyRule,
} from './lib.mjs';

const argv = process.argv.slice(2);
const opt = (name, env) => {
  const i = argv.indexOf(`--${name}`);
  if (i >= 0) {
    const v = argv[i + 1];
    argv.splice(i, 2);
    return v;
  }
  return env ? process.env[env] : undefined;
};
const flag = (name) => {
  const i = argv.indexOf(`--${name}`);
  if (i >= 0) argv.splice(i, 1);
  return i >= 0;
};

const mode = opt('mode', 'KINORA_SIGN_MODE') ?? 'dry-run';
const thumbprint = opt('thumbprint', 'KINORA_SIGN_THUMBPRINT');
const dlib = opt('dlib', 'KINORA_SIGN_DLIB');
const metadata = opt('metadata', 'KINORA_SIGN_METADATA');
const pfx = opt('pfx', 'KINORA_SIGN_PFX');
const description = opt('description') ?? 'Kinora';
const force = flag('force');
const allowTest = flag('allow-test');
const targets = argv;

const policy = await loadPolicy();
const fail = (msg) => {
  console.error(`sign: ${msg}`);
  process.exit(1);
};
if (targets.length === 0) fail('no files given');

// The key-selection part of the signtool command line, per mode.
let keyArgs;
switch (mode) {
  case 'store':
    if (!/^[0-9a-fA-F]{40}$/.test(thumbprint ?? ''))
      fail('store mode needs --thumbprint (40 hex digits)');
    keyArgs = ['/sha1', thumbprint];
    break;
  case 'artifact-signing':
    if (!dlib || !metadata) fail('artifact-signing mode needs --dlib and --metadata');
    keyArgs = ['/dlib', dlib, '/dmdf', metadata];
    break;
  case 'pfx':
    if (!pfx) fail('pfx mode needs --pfx');
    keyArgs = [
      '/f',
      pfx,
      ...(process.env.KINORA_SIGN_PFX_PASSWORD ? ['/p', process.env.KINORA_SIGN_PFX_PASSWORD] : []),
    ];
    break;
  case 'dry-run':
    keyArgs = ['/sha1', '<thumbprint>'];
    break;
  default:
    fail(`unknown mode ${mode}`);
}

const signtool = findSigntool();
if (!signtool && mode !== 'dry-run')
  fail('signtool.exe not found (install the Windows SDK or set KINORA_SIGNTOOL)');

const files = collectPe(targets);
if (files.length === 0) fail('no PE files among the given paths');
const before = readSignatures(files);

let failures = 0;
for (const s of before) {
  const name = basename(s.path);
  const valid = s.status === 'Valid';
  const rule = thirdPartyRule(policy, s.path);
  if (valid && rule && new RegExp(rule.signer).test(s.signer)) {
    console.log(`skip   ${name}: signed by ${rule.name}`);
    continue;
  }
  if (s.signer && isOurs(policy, s.signer, { allowTest }) && !force) {
    console.log(`skip   ${name}: already signed by us (${s.status}); use --force to re-sign`);
    continue;
  }
  if (s.signer && !isOurs(policy, s.signer, { allowTest }) && !force) {
    console.error(`refuse ${name}: carries a foreign signature (${s.signer}, ${s.status})`);
    failures++;
    continue;
  }

  let signed = false;
  for (const tsa of policy.timestampServers) {
    const args = [
      'sign',
      ...keyArgs,
      '/fd',
      policy.digest,
      '/tr',
      tsa,
      '/td',
      policy.digest,
      '/d',
      description,
      s.path,
    ];
    if (mode === 'dry-run') {
      console.log(`would  ${['signtool', ...args].join(' ')}`);
      signed = true;
      break;
    }
    const r = spawnSync(signtool, args, { encoding: 'utf8' });
    if (r.status === 0) {
      console.log(`signed ${name} (timestamp: ${tsa})`);
      signed = true;
      break;
    }
    const why = (r.stderr || r.stdout).trim().split(/\r?\n/).pop();
    console.error(`retry  ${name}: ${tsa} failed: ${why}`);
  }
  if (!signed) {
    console.error(`FAILED ${name}: no timestamp server accepted the request`);
    failures++;
  }
}

process.exit(failures ? 1 : 0);
