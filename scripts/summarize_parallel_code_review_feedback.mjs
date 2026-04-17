#!/usr/bin/env node

import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { readJsonSync, repoRootFromImportMeta } from './lib/script-artifacts.mjs';

const repoRoot = repoRootFromImportMeta(import.meta.url, 1);

const inputPath =
  process.env.INPUT_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-review-verdicts.json');
const outputPath =
  process.env.OUTPUT_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-review-verdicts.md');

function summarizeCounts(verdicts) {
  return summarizeFieldCounts(verdicts, function readCategory(verdict) {
    return verdict.category ?? 'uncategorized';
  });
}

function summarizeFieldCounts(verdicts, fieldValue) {
  const counts = new Map();
  for (const verdict of verdicts) {
    const value = fieldValue(verdict);
    counts.set(value, (counts.get(value) ?? 0) + 1);
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

function appendCountSection(lines, title, counts) {
  lines.push(title);
  lines.push('');
  for (const [value, count] of counts) {
    lines.push(`- \`${value}\`: ${count}`);
  }
  lines.push('');
}

function buildMarkdown(payload) {
  const lines = [];
  const verdicts = payload.verdicts ?? [];
  const preferredPairs = summarizePreferredPairs(verdicts);
  lines.push('# Parallel-Code Review Verdicts');
  lines.push('');
  lines.push(`Repo: \`${payload.repo}\``);
  lines.push(`Captured at: \`${payload.captured_at}\``);
  lines.push(`Source report: \`${payload.source_report}\``);
  lines.push('');
  appendCountSection(lines, '## Category Counts', summarizeCounts(verdicts));
  appendCountSection(
    lines,
    '## Expected Trust Tiers',
    summarizeFieldCounts(verdicts, function readTrustTier(verdict) {
      return verdict.expected_trust_tier ?? 'unspecified';
    }),
  );
  appendCountSection(
    lines,
    '## Expected Presentation Classes',
    summarizeFieldCounts(verdicts, function readPresentationClass(verdict) {
      return verdict.expected_presentation_class ?? 'unspecified';
    }),
  );
  appendCountSection(
    lines,
    '## Expected Leverage Classes',
    summarizeFieldCounts(verdicts, function readLeverageClass(verdict) {
      return verdict.expected_leverage_class ?? 'unspecified';
    }),
  );
  appendCountSection(
    lines,
    '## Expected Summary Presence',
    summarizeFieldCounts(verdicts, function readSummaryPresence(verdict) {
      return verdict.expected_summary_presence ?? 'unspecified';
    }),
  );
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
  for (const verdict of verdicts) {
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
  const payload = readJsonSync(inputPath);
  const markdown = buildMarkdown(payload);
  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, markdown, 'utf8');
  console.log(`Wrote review verdict summary to ${outputPath}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
