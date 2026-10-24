// Turns the tail of failed build logs into GitHub Actions error annotations.
//
// Why: job logs of a public repository can only be read by signed-in users
// (and the REST endpoint wants admin rights), while annotations are public
// through the check-runs API. Reporting the failure there lets anyone, and
// any tooling without credentials, see why a CI job failed.
//
// Usage (workflow step with `if: failure()`):
//   node scripts/ci/report-failure.mjs <log> [<log>...]
// Missing or empty logs are skipped. GitHub keeps at most 10 error
// annotations per step, so each log contributes up to 3 chunks of 60 lines.

import { existsSync, readFileSync } from 'node:fs';
import { basename } from 'node:path';

const LINES_PER_CHUNK = 60;
const CHUNKS_PER_LOG = 3;

// Workflow-command escaping for the message part (see GitHub's toolkit).
const escapeData = (s) => s.replaceAll('%', '%25').replaceAll('\r', '').replaceAll('\n', '%0A');
const escapeProperty = (s) => escapeData(s).replaceAll(':', '%3A').replaceAll(',', '%2C');

// Strips ANSI colour codes, which only add noise to an annotation.
// eslint-disable-next-line no-control-regex
const plain = (s) => s.replace(/\x1b\[[0-9;]*[A-Za-z]/g, '');

for (const path of process.argv.slice(2)) {
  if (!existsSync(path)) continue;
  const lines = plain(readFileSync(path, 'utf8'))
    .split(/\r?\n/)
    .filter((l) => l.trim() !== '');
  if (lines.length === 0) continue;
  const tail = lines.slice(-LINES_PER_CHUNK * CHUNKS_PER_LOG);
  const chunks = [];
  for (let i = 0; i < tail.length; i += LINES_PER_CHUNK)
    chunks.push(tail.slice(i, i + LINES_PER_CHUNK));
  chunks.forEach((chunk, i) => {
    const title = `${basename(path)}: last lines, part ${i + 1}/${chunks.length}`;
    console.log(`::error title=${escapeProperty(title)}::${escapeData(chunk.join('\n'))}`);
  });
}
