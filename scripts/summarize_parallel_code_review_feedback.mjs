#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');

const inputPath =
  process.env.INPUT_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-review-verdicts.json');
const outputPath =
  process.env.OUTPUT_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-review-verdicts.md');

function readJson(targetPath) {
  return JSON.parse(readFileSync(targetPath, 'utf8'));
}

function summarizeCounts(verdicts) {
  const counts = new Map();
  for (const verdict of verdicts) {
    const category = verdict.category ?? 'uncategorized';
    counts.set(category, (counts.get(category) ?? 0) + 1);
  }
  return [...counts.entries()].sort(([left], [right]) => left.localeCompare(right));
}

function buildMarkdown(payload) {
  const lines = [];
  lines.push('# Parallel-Code Review Verdicts');
  lines.push('');
  lines.push(`Repo: \`${payload.repo}\``);
  lines.push(`Captured at: \`${payload.captured_at}\``);
  lines.push(`Source report: \`${payload.source_report}\``);
  lines.push('');
  lines.push('## Category Counts');
  lines.push('');
  for (const [category, count] of summarizeCounts(payload.verdicts ?? [])) {
    lines.push(`- \`${category}\`: ${count}`);
  }
  lines.push('');
  lines.push('## Detailed Verdicts');
  lines.push('');
  for (const verdict of payload.verdicts ?? []) {
    lines.push(`### ${verdict.scope}`);
    lines.push('');
    lines.push(`- kind: \`${verdict.kind}\``);
    lines.push(`- category: \`${verdict.category}\``);
    lines.push(`- report bucket: \`${verdict.report_bucket}\``);
    lines.push(`- engineer note: ${verdict.engineer_note}`);
    lines.push(`- expected v2 behavior: ${verdict.expected_v2_behavior}`);
    lines.push('');
  }
  return `${lines.join('\n')}\n`;
}

async function main() {
  const payload = readJson(inputPath);
  const markdown = buildMarkdown(payload);
  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, markdown, 'utf8');
  console.log(`Wrote review verdict summary to ${outputPath}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
