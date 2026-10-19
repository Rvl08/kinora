// Fails when an installed npm package carries a licence outside the allowed
// classes of docs/05-license-compliance.md. Mirrors deny.toml for Rust.
//
// Usage: pnpm run licenses:check   (after `pnpm install`)

import { execFileSync } from 'node:child_process';

const ALLOWED = new Set([
  'MIT',
  'MIT-0',
  'Apache-2.0',
  'BSD-2-Clause',
  'BSD-3-Clause',
  'ISC',
  '0BSD',
  'Zlib',
  'BlueOak-1.0.0',
  'CC0-1.0',
  'CC-BY-4.0',
  'Unlicense',
  'Python-2.0',
  'MPL-2.0',
]);

/** Evaluates a simple SPDX expression: `A OR B` passes if either does, `A AND B` if both do. */
function isAllowed(expression) {
  const expr = expression.trim().replace(/^\((.*)\)$/, '$1');
  if (/\sOR\s/.test(expr)) return expr.split(/\sOR\s/).some(isAllowed);
  if (/\sAND\s/.test(expr)) return expr.split(/\sAND\s/).every(isAllowed);
  return ALLOWED.has(expr);
}

// Under `pnpm run`, npm_execpath is the running pnpm (also when it comes from
// Corepack and is not on PATH): a native binary since pnpm 11, a JS file before.
const args = ['licenses', 'list', '--json', '--dev'];
const pnpm = process.env.npm_execpath;
if (!pnpm) {
  console.error('Run this through pnpm: `pnpm run licenses:check`.');
  process.exit(2);
}
const output = /\.[cm]?js$/.test(pnpm)
  ? execFileSync(process.execPath, [pnpm, ...args], { encoding: 'utf8' })
  : execFileSync(pnpm, args, { encoding: 'utf8' });
const byLicense = JSON.parse(output);

const rejected = [];
let count = 0;
for (const [license, packages] of Object.entries(byLicense)) {
  for (const pkg of packages) {
    count += 1;
    if (!isAllowed(license)) rejected.push(`${pkg.name}@${pkg.versions.join(',')}: ${license}`);
  }
}

if (rejected.length > 0) {
  console.error(`Disallowed licences (${rejected.length} of ${count} packages):`);
  for (const line of rejected) console.error(`  ${line}`);
  console.error('See docs/05-license-compliance.md.');
  process.exit(1);
}
console.log(`Licences OK: ${count} packages checked.`);
