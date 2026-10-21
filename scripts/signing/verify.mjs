// Verifies that every PE file in a release is signed as ADR-0007 requires.
// Smart App Control checks each exe and dll it loads, so one unsigned DLL is
// enough to break Kinora on a protected machine.
//
// Usage: node scripts/signing/verify.mjs [--allow-test] [--json out.json] <file|dir>...
//
// A file passes when it is either
//   - ours: valid signature by a publisher in policy.json, SHA-256 file digest,
//     RFC 3161 timestamp (so it stays valid after the certificate expires); or
//   - an allowed third-party binary: its name matches a policy.json rule and
//     its valid signature is by that rule's signer.
// Anything else (unsigned, tampered, untrusted chain, unknown signer) fails.

import { writeFile } from 'node:fs/promises';
import { basename } from 'node:path';
import {
  collectPe,
  digestAlgorithms,
  findSigntool,
  isOurs,
  loadPolicy,
  readSignatures,
  thirdPartyRule,
} from './lib.mjs';

const argv = process.argv.slice(2);
const allowTest = argv.includes('--allow-test');
const jsonAt = argv.indexOf('--json');
const jsonOut = jsonAt >= 0 ? argv[jsonAt + 1] : null;
const targets = argv.filter((a, i) => !a.startsWith('--') && (jsonAt < 0 || i !== jsonAt + 1));
if (targets.length === 0) {
  console.error('usage: verify.mjs [--allow-test] [--json out.json] <file|dir>...');
  process.exit(2);
}

const policy = await loadPolicy();
const signtool = findSigntool();
const files = collectPe(targets);
const results = [];

for (const s of readSignatures(files)) {
  const name = basename(s.path);
  const problems = [];
  let kind;
  const rule = thirdPartyRule(policy, s.path);
  if (s.signer && isOurs(policy, s.signer, { allowTest })) {
    kind = 'ours';
    if (s.status !== 'Valid') problems.push(`signature ${s.status}: ${s.message}`);
    if (!s.timestamper) problems.push('no timestamp');
    if (signtool) {
      const digests = digestAlgorithms(signtool, s.path);
      if (!digests.includes(policy.digest))
        problems.push(`file digest ${digests.join('+') || 'unknown'}, want ${policy.digest}`);
    }
  } else if (rule) {
    kind = `third-party: ${rule.name}`;
    if (s.status !== 'Valid') problems.push(`signature ${s.status}: ${s.message}`);
    else if (!new RegExp(rule.signer).test(s.signer))
      problems.push(`signed by ${s.signer}, want ${rule.signer}`);
  } else {
    kind = 'unknown';
    problems.push(
      s.signer ? `signer not allowed: ${s.signer} (${s.status})` : `not signed (${s.status})`,
    );
  }
  results.push({ ...s, name, kind, ok: problems.length === 0, problems });
}

for (const r of results) {
  console.log(
    `${r.ok ? 'ok  ' : 'FAIL'} ${r.name.padEnd(36)} ${r.kind}${r.ok ? '' : ` — ${r.problems.join('; ')}`}`,
  );
}
const failed = results.filter((r) => !r.ok).length;
console.log(`\n${results.length} PE files, ${failed} failing`);
if (jsonOut) await writeFile(jsonOut, JSON.stringify({ allowTest, results }, null, 2));
process.exit(failed || results.length === 0 ? 1 : 0);
