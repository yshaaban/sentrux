#!/usr/bin/env node

import { readFileSync } from 'node:fs';
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

function compactList(values, limit, mapper) {
  return (values ?? []).slice(0, limit).map(mapper);
}

function compactFinding(finding) {
  return {
    id: finding.id ?? null,
    kind: finding.kind ?? null,
    trust_tier: finding.trust_tier ?? null,
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

function compactDebtSignal(signal) {
  return {
    kind: signal.kind ?? null,
    trust_tier: signal.trust_tier ?? null,
    scope: signal.scope ?? null,
    signal_class: signal.signal_class ?? null,
    signal_families: signal.signal_families ?? [],
    severity: signal.severity ?? null,
    score_0_10000: signal.score_0_10000 ?? null,
    summary: signal.summary ?? null,
    candidate_split_axes: signal.candidate_split_axes ?? [],
    related_surfaces: signal.related_surfaces ?? [],
  };
}

function compactDebtCluster(cluster) {
  return {
    scope: cluster.scope ?? null,
    trust_tier: cluster.trust_tier ?? null,
    severity: cluster.severity ?? null,
    score_0_10000: cluster.score_0_10000 ?? null,
    summary: cluster.summary ?? null,
    signal_kinds: cluster.signal_kinds ?? [],
    signal_families: cluster.signal_families ?? [],
  };
}

function compactFindingDetail(detail) {
  return {
    kind: detail.kind ?? null,
    trust_tier: detail.trust_tier ?? null,
    scope: detail.scope ?? null,
    severity: detail.severity ?? null,
    summary: detail.summary ?? null,
    impact: detail.impact ?? null,
    inspection_focus: detail.inspection_focus ?? [],
    candidate_split_axes: detail.candidate_split_axes ?? [],
    related_surfaces: detail.related_surfaces ?? [],
  };
}

function compactWatchpoint(watchpoint) {
  return {
    kind: watchpoint.kind ?? null,
    trust_tier: watchpoint.trust_tier ?? null,
    scope: watchpoint.scope ?? watchpoint.concept_id ?? null,
    signal_families: watchpoint.signal_families ?? [],
    severity: watchpoint.severity ?? null,
    score_0_10000: watchpoint.score_0_10000 ?? null,
    summary: watchpoint.summary ?? null,
    impact: watchpoint.impact ?? null,
    candidate_split_axes: watchpoint.candidate_split_axes ?? [],
    related_surfaces: watchpoint.related_surfaces ?? [],
    clone_family_count: watchpoint.clone_family_count ?? 0,
    hotspot_count: watchpoint.hotspot_count ?? 0,
    missing_site_count: watchpoint.missing_site_count ?? 0,
    boundary_pressure_count: watchpoint.boundary_pressure_count ?? 0,
  };
}

function selectOwnershipTarget(snapshot) {
  return (
    snapshot.concept_summaries.find((summary) => summary.concept_id === 'task_git_status') ??
    snapshot.watchpoints.find((watchpoint) => watchpoint.scope === 'task_git_status') ??
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
  const cloneSignal = snapshot.debt_signals.find(
    (signal) => signal.kind === 'clone_family',
  );
  const duplicationCluster = snapshot.debt_clusters.find((cluster) =>
    (cluster.signal_families ?? []).includes('duplication'),
  );
  const hotspotSignal = snapshot.debt_signals.find(
    (signal) => signal.kind === 'hotspot',
  );
  const hotspotCluster = snapshot.debt_clusters.find((cluster) =>
    (cluster.signal_families ?? []).includes('coordination'),
  );

  return {
    clone_family: cloneSignal ?? null,
    duplication_cluster: duplicationCluster ?? null,
    hotspot: hotspotSignal ?? null,
    hotspot_cluster: hotspotCluster ?? null,
  };
}

function buildMarkdown(snapshot) {
  const lines = [];
  lines.push('# Parallel-Code Proof Snapshot');
  lines.push('');
  lines.push(`Generated from: \`${snapshot.generated_from.golden_dir}\``);
  lines.push(`Benchmark: \`${snapshot.generated_from.benchmark_path}\``);
  lines.push('');
  lines.push('## Freshness');
  lines.push('');
  lines.push(
    `- analysis mode: \`${snapshot.generated_from.metadata.analysis_mode ?? 'unknown'}\``,
  );
  lines.push(
    `- commit: \`${snapshot.generated_from.metadata.source_tree_identity?.commit ?? 'unknown'}\``,
  );
  lines.push(
    `- dirty paths: \`${
      snapshot.generated_from.metadata.source_tree_identity?.dirty_paths_count ?? 'unknown'
    }\``,
  );
  lines.push(
    `- dirty-path fingerprint: \`${
      snapshot.generated_from.metadata.source_tree_identity?.dirty_paths_fingerprint ?? 'unknown'
    }\``,
  );
  lines.push(
    `- tree fingerprint: \`${
      snapshot.generated_from.metadata.source_tree_identity?.tree_fingerprint ?? 'unknown'
    }\``,
  );
  lines.push(
    `- analyzed tree fingerprint: \`${
      snapshot.generated_from.metadata.analyzed_tree_identity?.tree_fingerprint ?? 'unknown'
    }\``,
  );
  lines.push(
    `- rules sha256: \`${snapshot.generated_from.metadata.rules_identity?.sha256 ?? 'unknown'}\``,
  );
  lines.push(
    `- binary sha256: \`${snapshot.generated_from.metadata.binary_identity?.sha256 ?? 'unknown'}\``,
  );
  if ((snapshot.generated_from.metadata.source_tree_identity?.dirty_paths ?? []).length > 0) {
    lines.push('- dirty path list:');
    for (const dirtyPath of snapshot.generated_from.metadata.source_tree_identity.dirty_paths) {
      lines.push(`  - \`${dirtyPath}\``);
    }
  } else {
    lines.push('- dirty path list: none');
  }
  lines.push('');
  lines.push('## Top Findings');
  lines.push('');
  for (const finding of snapshot.top_findings) {
    lines.push(
      `- \`${finding.trust_tier ?? 'trusted'}\` \`${finding.severity}\` \`${finding.kind}\` ${finding.concept_id ? `(${finding.concept_id}) ` : ''}${finding.summary}`,
    );
  }
  lines.push('');
  lines.push('## Experimental Findings');
  lines.push('');
  for (const finding of snapshot.experimental_findings) {
    lines.push(
      `- \`${finding.severity}\` \`${finding.kind}\` ${finding.summary}`,
    );
  }
  if (snapshot.experimental_findings.length === 0) {
    lines.push('- none');
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
  lines.push('## Finding Details');
  lines.push('');
  for (const detail of snapshot.finding_details) {
    lines.push(
      `- \`${detail.trust_tier ?? 'trusted'}\` \`${detail.severity}\` \`${detail.kind}\` \`${detail.scope}\`: ${detail.summary}`,
    );
    lines.push(`  - impact: ${detail.impact}`);
  }
  lines.push('');
  lines.push('## Debt Signals');
  lines.push('');
  for (const signal of snapshot.debt_signals) {
    lines.push(
      `- \`${signal.trust_tier}\` \`${signal.kind}\` \`${signal.scope}\` score ${signal.score_0_10000}: ${signal.summary}`,
    );
  }
  lines.push('');
  lines.push('## Experimental Debt Signals');
  lines.push('');
  for (const signal of snapshot.experimental_debt_signals) {
    lines.push(
      `- \`${signal.kind}\` \`${signal.scope}\` score ${signal.score_0_10000}: ${signal.summary}`,
    );
  }
  if (snapshot.experimental_debt_signals.length === 0) {
    lines.push('- none');
  }
  lines.push('');
  lines.push('## Debt Clusters');
  lines.push('');
  for (const cluster of snapshot.debt_clusters) {
    lines.push(
      `- \`${cluster.trust_tier}\` \`${cluster.scope}\` score ${cluster.score_0_10000}: ${cluster.summary}`,
    );
  }
  lines.push('');
  lines.push('## Watchpoints');
  lines.push('');
  for (const watchpoint of snapshot.watchpoints) {
    lines.push(
      `- \`${watchpoint.trust_tier ?? 'watchpoint'}\` \`${watchpoint.scope}\` score ${watchpoint.score_0_10000}: ${watchpoint.summary}`,
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
      snapshot.proof_targets.duplication_hotspot.clone_family?.scope ??
      snapshot.proof_targets.duplication_hotspot.duplication_cluster?.scope ??
      'n/a'
    } / hotspot ${
      snapshot.proof_targets.duplication_hotspot.hotspot?.scope ??
      snapshot.proof_targets.duplication_hotspot.hotspot_cluster?.scope ??
      'n/a'
    }`,
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
    top_findings: compactList(findings.findings, 10, compactFinding),
    experimental_findings: compactList(
      findings.experimental_findings,
      10,
      compactFinding,
    ),
    finding_details: compactList(findings.finding_details, 10, compactFindingDetail),
    concept_summaries: compactList(findings.concept_summaries, 5, compactConceptSummary),
    debt_signals: compactList(
      findings.debt_signals ?? findings.quality_opportunities,
      5,
      compactDebtSignal,
    ),
    experimental_debt_signals: compactList(
      findings.experimental_debt_signals,
      5,
      compactDebtSignal,
    ),
    debt_clusters: compactList(findings.debt_clusters, 5, compactDebtCluster),
    watchpoints: compactList(
      findings.watchpoints ?? findings.optimization_priorities,
      5,
      compactWatchpoint,
    ),
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
