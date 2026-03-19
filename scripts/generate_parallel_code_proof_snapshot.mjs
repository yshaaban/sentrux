#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertPathExists } from './lib/disposable-repo.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');

const goldenDir =
  process.env.GOLDEN_DIR ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-golden');
const benchmarkPath =
  process.env.BENCHMARK_PATH ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-benchmark.json');
const outputJsonPath =
  process.env.OUTPUT_JSON_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-proof-snapshot.json');
const outputMarkdownPath =
  process.env.OUTPUT_MD_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-proof-snapshot.md');

function readJson(targetPath) {
  return JSON.parse(readFileSync(targetPath, 'utf8'));
}

function compactFinding(finding) {
  return {
    id: finding.id ?? null,
    kind: finding.kind ?? null,
    severity: finding.severity ?? null,
    concept_id: finding.concept_id ?? null,
    summary: finding.summary ?? null,
  };
}

function compactConceptSummary(summary) {
  return {
    concept_id: summary.concept_id ?? null,
    score_0_10000: summary.score_0_10000 ?? null,
    summary: summary.summary ?? null,
    dominant_kinds: summary.dominant_kinds ?? [],
    boundary_pressure_count: summary.boundary_pressure_count ?? 0,
    missing_site_count: summary.missing_site_count ?? 0,
  };
}

function compactOpportunity(opportunity) {
  return {
    kind: opportunity.kind ?? null,
    scope: opportunity.scope ?? null,
    severity: opportunity.severity ?? null,
    score_0_10000: opportunity.score_0_10000 ?? null,
    summary: opportunity.summary ?? null,
  };
}

function compactPriority(priority) {
  return {
    concept_id: priority.concept_id ?? null,
    severity: priority.severity ?? null,
    score_0_10000: priority.score_0_10000 ?? null,
    summary: priority.summary ?? null,
    clone_family_count: priority.clone_family_count ?? 0,
    hotspot_count: priority.hotspot_count ?? 0,
    missing_site_count: priority.missing_site_count ?? 0,
    boundary_pressure_count: priority.boundary_pressure_count ?? 0,
  };
}

function selectOwnershipTarget(snapshot) {
  return (
    snapshot.concept_summaries.find((summary) => summary.concept_id === 'task_git_status') ??
    snapshot.optimization_priorities.find((priority) => priority.concept_id === 'task_git_status') ??
    null
  );
}

function selectPropagationTarget(snapshot, obligationsPayload) {
  const conceptSummary =
    snapshot.concept_summaries.find((summary) => summary.missing_site_count > 0) ?? null;
  const obligation =
    obligationsPayload.obligations?.find((entry) => entry.missing_sites?.length > 0) ?? null;

  return {
    concept_id: conceptSummary?.concept_id ?? obligation?.concept_id ?? 'task_presentation_status',
    summary: conceptSummary?.summary ?? obligation?.summary ?? null,
    missing_site_count:
      conceptSummary?.missing_site_count ?? obligationsPayload.missing_site_count ?? 0,
    obligation_kind: obligation?.kind ?? null,
    missing_sites: obligation?.missing_sites ?? [],
  };
}

function selectDuplicationTarget(snapshot) {
  const cloneOpportunity = snapshot.quality_opportunities.find(
    (opportunity) => opportunity.kind === 'clone_family',
  );
  const hotspotOpportunity = snapshot.quality_opportunities.find(
    (opportunity) => opportunity.kind === 'hotspot',
  );

  return {
    clone_family: cloneOpportunity ?? null,
    hotspot: hotspotOpportunity ?? null,
  };
}

function buildMarkdown(snapshot) {
  const lines = [];
  lines.push('# Parallel-Code Proof Snapshot');
  lines.push('');
  lines.push(`Generated from: \`${snapshot.generated_from.golden_dir}\``);
  lines.push(`Benchmark: \`${snapshot.generated_from.benchmark_path}\``);
  lines.push('');
  lines.push('## Top Findings');
  lines.push('');
  for (const finding of snapshot.top_findings) {
    lines.push(
      `- \`${finding.severity}\` \`${finding.kind}\` ${finding.concept_id ? `(${finding.concept_id}) ` : ''}${finding.summary}`,
    );
  }
  lines.push('');
  lines.push('## Concept Summaries');
  lines.push('');
  for (const summary of snapshot.concept_summaries) {
    lines.push(
      `- \`${summary.concept_id}\` score ${summary.score_0_10000}: ${summary.summary}`,
    );
  }
  lines.push('');
  lines.push('## Optimization Priorities');
  lines.push('');
  for (const priority of snapshot.optimization_priorities) {
    lines.push(
      `- \`${priority.concept_id}\` score ${priority.score_0_10000}: ${priority.summary}`,
    );
  }
  lines.push('');
  lines.push('## Proof Targets');
  lines.push('');
  lines.push(
    `1. Ownership/boundary: \`${snapshot.proof_targets.ownership_boundary?.concept_id ?? 'n/a'}\``,
  );
  lines.push(
    `2. Propagation/obligations: \`${snapshot.proof_targets.propagation_obligations.concept_id}\``,
  );
  lines.push(
    `3. Duplication/hotspot: clone ${
      snapshot.proof_targets.duplication_hotspot.clone_family?.scope ?? 'n/a'
    } / hotspot ${snapshot.proof_targets.duplication_hotspot.hotspot?.scope ?? 'n/a'}`,
  );
  lines.push('');
  lines.push('## Benchmark Baseline');
  lines.push('');
  lines.push(
    `- cold process total: ${snapshot.benchmark.cold_process_total_ms ?? 'n/a'} ms`,
  );
  lines.push(
    `- warm cached total: ${snapshot.benchmark.warm_cached_total_ms ?? 'n/a'} ms`,
  );
  lines.push(
    `- warm patch-safety total: ${snapshot.benchmark.warm_patch_safety_total_ms ?? 'n/a'} ms`,
  );

  return `${lines.join('\n')}\n`;
}

async function main() {
  assertPathExists(goldenDir, 'parallel-code golden directory');
  assertPathExists(benchmarkPath, 'parallel-code benchmark artifact');

  const findingsPath = path.join(goldenDir, 'findings-top12.json');
  const obligationsPath = path.join(goldenDir, 'obligations-task_presentation_status.json');
  const metadataPath = path.join(goldenDir, 'metadata.json');
  assertPathExists(findingsPath, 'parallel-code findings snapshot');
  assertPathExists(obligationsPath, 'parallel-code obligations snapshot');
  assertPathExists(metadataPath, 'parallel-code metadata snapshot');

  const findings = readJson(findingsPath);
  const obligations = readJson(obligationsPath);
  const metadata = readJson(metadataPath);
  const benchmark = readJson(benchmarkPath);

  const snapshot = {
    generated_at: new Date().toISOString(),
    generated_from: {
      golden_dir: goldenDir,
      benchmark_path: benchmarkPath,
      metadata,
    },
    top_findings: (findings.findings ?? []).slice(0, 10).map(compactFinding),
    concept_summaries: (findings.concept_summaries ?? []).slice(0, 5).map(compactConceptSummary),
    quality_opportunities: (findings.quality_opportunities ?? [])
      .slice(0, 5)
      .map(compactOpportunity),
    optimization_priorities: (findings.optimization_priorities ?? [])
      .slice(0, 5)
      .map(compactPriority),
    proof_targets: null,
    benchmark: {
      cold_process_total_ms: benchmark.benchmark?.cold_process_total_ms ?? null,
      warm_cached_total_ms: benchmark.benchmark?.warm_cached_total_ms ?? null,
      warm_patch_safety_total_ms: benchmark.benchmark?.warm_patch_safety_total_ms ?? null,
      warm_gate_ms: benchmark.benchmark?.warm_patch_safety?.gate?.elapsed_ms ?? null,
      warm_session_end_ms: benchmark.benchmark?.warm_patch_safety?.session_end?.elapsed_ms ?? null,
    },
  };

  snapshot.proof_targets = {
    ownership_boundary: selectOwnershipTarget(snapshot),
    propagation_obligations: selectPropagationTarget(snapshot, obligations),
    duplication_hotspot: selectDuplicationTarget(snapshot),
  };

  await mkdir(path.dirname(outputJsonPath), { recursive: true });
  await writeFile(outputJsonPath, `${JSON.stringify(snapshot, null, 2)}\n`, 'utf8');
  await writeFile(outputMarkdownPath, buildMarkdown(snapshot), 'utf8');

  console.log(`Wrote proof snapshot JSON to ${outputJsonPath}`);
  console.log(`Wrote proof snapshot Markdown to ${outputMarkdownPath}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
