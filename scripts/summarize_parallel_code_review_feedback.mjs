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

function summarizeTrustTierCounts(verdicts) {
  const counts = new Map();
  for (const verdict of verdicts) {
    const tier = verdict.expected_trust_tier ?? 'unspecified';
    counts.set(tier, (counts.get(tier) ?? 0) + 1);
  }
  return [...counts.entries()].sort(([left], [right]) => left.localeCompare(right));
}

function summarizePresentationClassCounts(verdicts) {
  const counts = new Map();
  for (const verdict of verdicts) {
    const presentationClass = verdict.expected_presentation_class ?? 'unspecified';
    counts.set(presentationClass, (counts.get(presentationClass) ?? 0) + 1);
  }
  return [...counts.entries()].sort(([left], [right]) => left.localeCompare(right));
}

function summarizeLeverageClassCounts(verdicts) {
  const counts = new Map();
  for (const verdict of verdicts) {
    const leverageClass = verdict.expected_leverage_class ?? 'unspecified';
    counts.set(leverageClass, (counts.get(leverageClass) ?? 0) + 1);
  }
  return [...counts.entries()].sort(([left], [right]) => left.localeCompare(right));
}

function summarizeSummaryPresenceCounts(verdicts) {
  const counts = new Map();
  for (const verdict of verdicts) {
    const presence = verdict.expected_summary_presence ?? 'unspecified';
    counts.set(presence, (counts.get(presence) ?? 0) + 1);
  }
  return [...counts.entries()].sort(([left], [right]) => left.localeCompare(right));
}

function summarizePreferredPairs(verdicts) {
  const pairs = [];
  for (const verdict of verdicts) {
    for (const preferredScope of verdict.preferred_over ?? []) {
      pairs.push([verdict.scope, preferredScope]);
    }
  }
  return pairs.sort(([leftScope, leftPreferred], [rightScope, rightPreferred]) => {
    return leftScope.localeCompare(rightScope) || leftPreferred.localeCompare(rightPreferred);
  });
}

function buildMarkdown(payload) {
  const lines = [];
  const preferredPairs = summarizePreferredPairs(payload.verdicts ?? []);
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
  lines.push('## Expected Trust Tiers');
  lines.push('');
  for (const [tier, count] of summarizeTrustTierCounts(payload.verdicts ?? [])) {
    lines.push(`- \`${tier}\`: ${count}`);
  }
  lines.push('');
  lines.push('## Expected Presentation Classes');
  lines.push('');
  for (const [presentationClass, count] of summarizePresentationClassCounts(payload.verdicts ?? [])) {
    lines.push(`- \`${presentationClass}\`: ${count}`);
  }
  lines.push('');
  lines.push('## Expected Leverage Classes');
  lines.push('');
  for (const [leverageClass, count] of summarizeLeverageClassCounts(payload.verdicts ?? [])) {
    lines.push(`- \`${leverageClass}\`: ${count}`);
  }
  lines.push('');
  lines.push('## Expected Summary Presence');
  lines.push('');
  for (const [presence, count] of summarizeSummaryPresenceCounts(payload.verdicts ?? [])) {
    lines.push(`- \`${presence}\`: ${count}`);
  }
  lines.push('');
  lines.push('## Ranking Preferences');
  lines.push('');
  for (const [scope, preferredScope] of preferredPairs) {
    lines.push(`- \`${scope}\` should rank ahead of \`${preferredScope}\``);
  }
  if (preferredPairs.length === 0) {
    lines.push('- none');
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
    lines.push(`- expected trust tier: \`${verdict.expected_trust_tier ?? 'unspecified'}\``);
    lines.push(
      `- expected presentation class: \`${verdict.expected_presentation_class ?? 'unspecified'}\``,
    );
    lines.push(`- expected leverage class: \`${verdict.expected_leverage_class ?? 'unspecified'}\``);
    lines.push(`- expected summary presence: \`${verdict.expected_summary_presence ?? 'unspecified'}\``);
    if ((verdict.preferred_over ?? []).length > 0) {
      lines.push(`- preferred over: \`${verdict.preferred_over.join(', ')}\``);
    }
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
