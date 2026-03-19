#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  assertFileIdentityFresh,
  assertRepoIdentityFresh,
  collectFileIdentity,
  collectRepoIdentity,
} from './lib/repo-identity.mjs';
import { assertPathExists } from './lib/disposable-repo.mjs';
import { selectPresentationBuckets } from './lib/parallel-code-reporting.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const parallelCodeRoot = process.env.PARALLEL_CODE_ROOT ?? '<parallel-code-root>';
const goldenDir =
  process.env.GOLDEN_DIR ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-golden');
const benchmarkPath =
  process.env.BENCHMARK_PATH ?? path.join(repoRoot, 'docs/v2/examples/parallel-code-benchmark.json');
const snapshotJsonPath =
  process.env.SNAPSHOT_JSON_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-proof-snapshot.json');
const reportMarkdownPath =
  process.env.OUTPUT_REPORT_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-live-engineer-report.md');
const appendixMarkdownPath =
  process.env.OUTPUT_APPENDIX_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-live-engineer-report-appendix.md');
const snapshotMarkdownPath =
  process.env.OUTPUT_SNAPSHOT_MD_PATH ??
  path.join(repoRoot, 'docs/v2/examples/parallel-code-proof-snapshot.md');
const allowStaleGoldens =
  process.env.ALLOW_STALE_GOLDENS === '1' || process.argv.includes('--allow-stale-goldens');

function readJson(targetPath) {
  return JSON.parse(readFileSync(targetPath, 'utf8'));
}

function formatUtcDate(timestamp) {
  const date = new Date(timestamp);
  return new Intl.DateTimeFormat('en-US', {
    month: 'long',
    day: 'numeric',
    year: 'numeric',
    timeZone: 'UTC',
  }).format(date);
}

function formatIdentity(metadata) {
  const identity = metadata?.source_tree_identity ?? {};
  return {
    analysis_mode: metadata?.analysis_mode ?? identity.analysis_mode ?? 'unknown',
    commit: identity.commit ?? 'unknown',
    dirty_paths_count: identity.dirty_paths_count ?? 'unknown',
    dirty_paths: identity.dirty_paths ?? [],
    dirty_paths_fingerprint: identity.dirty_paths_fingerprint ?? 'unknown',
    tree_fingerprint: identity.tree_fingerprint ?? 'unknown',
  };
}

function snapshotMatchesMetadata(snapshot, metadata) {
  const snapshotMetadata = snapshot?.generated_from?.metadata;
  if (!snapshotMetadata) {
    return false;
  }

  return JSON.stringify(snapshotMetadata) === JSON.stringify(metadata);
}

function appendCodeBullet(lines, label, value) {
  lines.push(`- ${label}: \`${value}\``);
}

function appendCodeList(lines, title, values) {
  if ((values ?? []).length === 0) {
    return;
  }

  lines.push(`- ${title}:`);
  for (const value of values) {
    lines.push(`  - \`${value}\``);
  }
}

function formatRepoPathMarkdown(repoPath, targetPath) {
  return `[${path.basename(targetPath)}](${path.join(repoPath, targetPath)})`;
}

function appendRepoLinkList(lines, title, repoPath, surfaces, limit = 5) {
  if ((surfaces ?? []).length === 0) {
    return;
  }

  lines.push(`- ${title}:`);
  for (const surface of surfaces.slice(0, limit)) {
    lines.push(`  - ${formatRepoPathMarkdown(repoPath, surface)}`);
  }
}

function appendCandidateBlock(lines, candidate, repoRoot) {
  if (looksLikeRepoPath(candidate.scope)) {
    lines.push(`### ${formatRepoPathMarkdown(repoRoot, candidate.scope)}`);
  } else {
    lines.push(`### ${candidate.scope}`);
  }
  lines.push('');
  appendCodeBullet(lines, 'class', candidate.presentation_class ?? 'structural_debt');
  appendCodeBullet(lines, 'kind', candidate.kind ?? 'unknown');
  appendCodeBullet(lines, 'severity', candidate.severity ?? 'unknown');
  lines.push(`- summary: ${candidate.summary}`);
  if (candidate.impact) {
    lines.push(`- impact: ${candidate.impact}`);
  }
  appendCodeList(lines, 'candidate split axes', candidate.candidate_split_axes);
  appendRepoLinkList(lines, 'related surfaces', repoRoot, candidate.related_surfaces, 5);
  lines.push('');
}

function appendScanCoverage(lines, scan, { includeBuckets = false, includeSessionBaseline = false } = {}) {
  const exclusionBuckets =
    scan.scan_trust?.exclusions?.by_category ?? scan.scan_trust?.exclusions?.bucketed;

  appendCodeBullet(lines, 'scanned files', scan.files ?? 'n/a');
  appendCodeBullet(lines, 'scanned lines', scan.lines ?? 'n/a');
  appendCodeBullet(
    lines,
    'kept files from git candidate set',
    `${scan.scan_trust?.kept_files ?? 'n/a'} / ${scan.scan_trust?.candidate_files ?? 'n/a'}`,
  );
  appendCodeBullet(lines, 'excluded files', scan.scan_trust?.exclusions?.total ?? 'n/a');
  if (includeBuckets && exclusionBuckets) {
    lines.push('- excluded buckets:');
    for (const [bucket, count] of Object.entries(exclusionBuckets)) {
      lines.push(`  - ${bucket}: \`${count}\``);
    }
  }
  appendCodeBullet(lines, 'resolved imports', scan.scan_trust?.resolution?.resolved ?? 'n/a');
  appendCodeBullet(
    lines,
    'unresolved internal imports',
    scan.scan_trust?.resolution?.unresolved_internal ?? 'n/a',
  );
  appendCodeBullet(
    lines,
    'unresolved external imports',
    scan.scan_trust?.resolution?.unresolved_external ?? 'n/a',
  );
  appendCodeBullet(
    lines,
    'unresolved unknown imports',
    scan.scan_trust?.resolution?.unresolved_unknown ?? 'n/a',
  );
  appendCodeBullet(
    lines,
    'scan confidence',
    `${scan.confidence?.scan_confidence_0_10000 ?? 'n/a'} / 10000`,
  );
  appendCodeBullet(
    lines,
    'rule coverage',
    `${scan.confidence?.rule_coverage_0_10000 ?? 'n/a'} / 10000`,
  );
  appendCodeBullet(
    lines,
    'semantic rules loaded',
    scan.confidence?.semantic_rules_loaded ? 'true' : 'false',
  );
  if (includeSessionBaseline) {
    appendCodeBullet(
      lines,
      'session baseline loaded in `findings`',
      scan.session_baseline_loaded ? 'true' : 'false',
    );
  }
}

function finalizeMarkdown(lines) {
  const output = [...lines];
  while (output.length > 0 && output.at(-1) === '') {
    output.pop();
  }

  return output.join('\n');
}

function buildProofSnapshotMarkdown(snapshot) {
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
      `- \`${finding.trust_tier ?? 'trusted'}\` \`${finding.presentation_class ?? 'structural_debt'}\` \`${finding.severity}\` \`${finding.kind}\` ${
        finding.concept_id ? `(${finding.concept_id}) ` : ''
      }${finding.summary}`,
    );
  }
  lines.push('');
  lines.push('## Experimental Findings');
  lines.push('');
  for (const finding of snapshot.experimental_findings) {
    lines.push(`- \`${finding.severity}\` \`${finding.kind}\` ${finding.summary}`);
  }
  if (snapshot.experimental_findings.length === 0) {
    lines.push('- none');
  }
  lines.push('');
  lines.push('## Concept Summaries');
  lines.push('');
  for (const summary of snapshot.concept_summaries) {
    lines.push(`- \`${summary.concept_id}\` score ${summary.score_0_10000}: ${summary.summary}`);
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
    lines.push(`- \`${signal.kind}\` \`${signal.scope}\` score ${signal.score_0_10000}: ${signal.summary}`);
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
  lines.push(`- cold process total: ${snapshot.benchmark.cold_process_total_ms ?? 'n/a'} ms`);
  lines.push(`- warm cached total: ${snapshot.benchmark.warm_cached_total_ms ?? 'n/a'} ms`);
  lines.push(
    `- warm patch-safety total: ${snapshot.benchmark.warm_patch_safety_total_ms ?? 'n/a'} ms`,
  );

  return finalizeMarkdown(lines);
}

function buildLiveEngineerReport({
  snapshot,
  findings,
  scan,
  benchmark,
  metadata,
  liveIdentity,
  allowStale,
}) {
  const freshness = formatIdentity(metadata);
  const presentationBuckets = selectPresentationBuckets(findings);
  const leadCandidates = presentationBuckets.lead_candidates;
  const secondaryHotspots = presentationBuckets.secondary_hotspots;
  const hardeningNotes = presentationBuckets.hardening_notes;
  const toolingDebt = presentationBuckets.tooling_debt;
  const lines = [];
  lines.push('# Parallel Code: Live Analysis Report For Engineers');
  lines.push('');
  lines.push(
    `Generated on ${formatUtcDate(snapshot.generated_at)} from the live checkout at \`${metadata.parallel_code_root}\`.`,
  );
  lines.push('');
  lines.push('This report is for an engineer who does not already know `parallel-code` or Sentrux.');
  lines.push('');
  lines.push('## Freshness Gate');
  lines.push('');
  lines.push(`- analysis mode: \`${freshness.analysis_mode}\``);
  lines.push(`- commit: \`${freshness.commit}\``);
  lines.push(`- dirty paths: \`${freshness.dirty_paths_count}\``);
  lines.push(`- dirty-path fingerprint: \`${freshness.dirty_paths_fingerprint}\``);
  lines.push(`- tree fingerprint: \`${freshness.tree_fingerprint}\``);
  lines.push(
    `- stale goldens: ${allowStale ? 'accepted via override' : 'refused by default unless the goldens are fresh'}`,
  );
  lines.push('');
  lines.push('## What Was Analyzed');
  lines.push('');
  lines.push(`- live source checkout: \`${metadata.parallel_code_root}\``);
  lines.push(`- rules file used for the run: \`${metadata.rules_source}\``);
  lines.push(`- comparison snapshot: \`${snapshotJsonPath}\``);
  lines.push(`- benchmark artifact: \`${benchmarkPath}\``);
  lines.push('');
  lines.push('## Scan Coverage');
  lines.push('');
  appendCodeBullet(lines, 'scanned source files', scan.files ?? 'n/a');
  appendCodeBullet(lines, 'scanned lines', scan.lines ?? 'n/a');
  appendCodeBullet(
    lines,
    'git candidate files kept',
    `${scan.scan_trust?.kept_files ?? 'n/a'} / ${scan.scan_trust?.candidate_files ?? 'n/a'}`,
  );
  appendCodeBullet(lines, 'excluded files', scan.scan_trust?.exclusions?.total ?? 'n/a');
  appendCodeBullet(lines, 'resolved import edges', scan.scan_trust?.resolution?.resolved ?? 'n/a');
  appendCodeBullet(
    lines,
    'unresolved internal imports',
    scan.scan_trust?.resolution?.unresolved_internal ?? 'n/a',
  );
  appendCodeBullet(
    lines,
    'unresolved external imports',
    scan.scan_trust?.resolution?.unresolved_external ?? 'n/a',
  );
  appendCodeBullet(
    lines,
    'unresolved unknown imports',
    scan.scan_trust?.resolution?.unresolved_unknown ?? 'n/a',
  );
  appendCodeBullet(
    lines,
    'scan confidence',
    `${scan.confidence?.scan_confidence_0_10000 ?? 'n/a'} / 10000`,
  );
  appendCodeBullet(
    lines,
    'rule coverage',
    `${scan.confidence?.rule_coverage_0_10000 ?? 'n/a'} / 10000`,
  );
  appendCodeBullet(
    lines,
    'semantic rules loaded',
    scan.confidence?.semantic_rules_loaded ? 'true' : 'false',
  );
  lines.push('');
  lines.push('## Executive Summary');
  lines.push('');
  lines.push('The current live repo surfaces these primary pressure points:');
  lines.push('');
  for (const candidate of leadCandidates) {
    lines.push(
      `- \`${candidate.presentation_class}\` \`${candidate.kind}\` ${candidate.summary}`,
    );
  }
  if (secondaryHotspots.length > 0) {
    lines.push(
      `- \`secondary_hotspot\` ${secondaryHotspots[0].summary}`,
    );
  }
  if (leadCandidates.length === 0 && secondaryHotspots.length === 0) {
    lines.push('- none');
  }
  lines.push('');
  lines.push('## Strongest Trusted Debt Signals');
  lines.push('');
  for (const candidate of leadCandidates) {
    appendCandidateBlock(lines, candidate, metadata.parallel_code_root);
  }

  lines.push('## Secondary Hotspots');
  lines.push('');
  for (const candidate of secondaryHotspots) {
    appendCandidateBlock(lines, candidate, metadata.parallel_code_root);
  }
  if (secondaryHotspots.length === 0) {
    lines.push('- none');
    lines.push('');
  }

  lines.push('## Targeted Hardening Notes');
  lines.push('');
  for (const candidate of hardeningNotes) {
    appendCandidateBlock(lines, candidate, metadata.parallel_code_root);
  }
  if (hardeningNotes.length === 0) {
    lines.push('- none');
    lines.push('');
  }

  lines.push('## Tooling Debt');
  lines.push('');
  for (const candidate of toolingDebt) {
    appendCandidateBlock(lines, candidate, metadata.parallel_code_root);
  }
  if (toolingDebt.length === 0) {
    lines.push('- none');
    lines.push('');
  }

  lines.push('## Watchpoints');
  lines.push('');
  const watchpoints = snapshot.watchpoints.slice(0, 4);
  for (const watchpoint of watchpoints) {
    lines.push(`- \`${watchpoint.trust_tier ?? 'watchpoint'}\` \`${watchpoint.kind}\` ${watchpoint.summary}`);
  }
  if (watchpoints.length === 0) {
    lines.push('- none');
  }
  lines.push('');
  lines.push('## Benchmark Baseline');
  lines.push('');
  lines.push(`- cold process total: ${benchmark.benchmark?.cold_process_total_ms ?? 'n/a'} ms`);
  lines.push(`- warm cached total: ${benchmark.benchmark?.warm_cached_total_ms ?? 'n/a'} ms`);
  lines.push(
    `- warm patch-safety total: ${benchmark.benchmark?.warm_patch_safety_total_ms ?? 'n/a'} ms`,
  );
  lines.push('');
  lines.push('## Freshness Check Result');
  lines.push('');
  lines.push(`- live commit: \`${liveIdentity.commit}\``);
  lines.push(`- live dirty paths: \`${liveIdentity.dirty_paths_count}\``);
  lines.push(`- live dirty-path fingerprint: \`${liveIdentity.dirty_paths_fingerprint}\``);
  lines.push(`- live tree fingerprint: \`${liveIdentity.tree_fingerprint}\``);
  lines.push(
    `- freshness comparison: ${
      allowStale ? 'override enabled' : 'goldens matched and report generation was allowed'
    }`,
  );
  lines.push('');
  lines.push('## Source Documents');
  lines.push('');
  lines.push(`- proof snapshot: \`${snapshotMarkdownPath}\``);
  lines.push(`- golden metadata: \`${path.join(goldenDir, 'metadata.json')}\``);

  return finalizeMarkdown(lines);
}

function looksLikeRepoPath(value) {
  if (typeof value !== 'string') {
    return false;
  }

  const repoPrefixes = ['src/', 'server/', 'electron/', 'scripts/', 'docs/'];
  return repoPrefixes.some((prefix) => value.startsWith(prefix));
}

function buildLiveEngineerAppendix({
  snapshot,
  findings,
  scan,
  metadata,
}) {
  const lines = [];
  const presentationBuckets = selectPresentationBuckets(findings);
  const leadCandidates = presentationBuckets.lead_candidates;
  const secondaryHotspots = presentationBuckets.secondary_hotspots;
  const hardeningNotes = presentationBuckets.hardening_notes;
  const toolingDebt = presentationBuckets.tooling_debt;
  const watchpoints = snapshot.watchpoints.slice(0, 6);
  const trustedClusters = snapshot.debt_clusters.filter((cluster) => cluster.trust_tier === 'trusted');
  const experimentalSignals = findings.experimental_debt_signals ?? snapshot.experimental_debt_signals ?? [];

  lines.push('# Parallel Code: Live Analysis Report Appendix');
  lines.push('');
  lines.push(
    `Generated on ${formatUtcDate(snapshot.generated_at)} from the live checkout at \`${metadata.parallel_code_root}\`.`,
  );
  lines.push('');
  lines.push('This appendix contains the evidence behind');
  lines.push(`[parallel-code-live-engineer-report.md](${reportMarkdownPath}).`);
  lines.push('');
  lines.push('## Method');
  lines.push('');
  lines.push('The analysis used:');
  lines.push('');
  lines.push(`- live source repo: [${metadata.parallel_code_root}](${metadata.parallel_code_root})`);
  lines.push(`- bundled rules file: [parallel-code.rules.toml](${metadata.rules_source})`);
  lines.push(
    `- goldens refresh path: [refresh_parallel_code_goldens.sh](${path.join(repoRoot, 'scripts/refresh_parallel_code_goldens.sh')})`,
  );
  lines.push(`- current binary used for the run: [${metadata.sentrux_binary}](${metadata.sentrux_binary})`);
  lines.push('');
  lines.push('Scope caveat:');
  lines.push('');
  lines.push('- the live repo has `.sentrux/baseline.json`');
  lines.push('- it does **not** currently have its own `.sentrux/rules.toml`');
  lines.push('- this run therefore still uses the bundled example rules');
  lines.push('');
  lines.push('## Scan Scope And Confidence');
  lines.push('');
  lines.push('Current scan:');
  lines.push('');
  appendScanCoverage(lines, scan, { includeBuckets: true, includeSessionBaseline: true });
  lines.push('');
  lines.push('## Lead Trusted Debt Signals');
  lines.push('');
  for (const candidate of leadCandidates) {
    if (looksLikeRepoPath(candidate.scope)) {
      lines.push(`### ${formatRepoPathMarkdown(metadata.parallel_code_root, candidate.scope)}`);
    } else {
      lines.push(`### ${candidate.scope}`);
    }
    lines.push('');
    lines.push('- `trusted`');
    lines.push(`- class: \`${candidate.presentation_class}\``);
    lines.push(`- \`${candidate.kind}\``);
    lines.push(`- summary: \`${candidate.summary}\``);
    lines.push(`- impact: ${candidate.impact}`);
    const detail = findings.finding_details.find((entry) => entry.scope === candidate.scope && entry.kind === candidate.kind);
    lines.push('- evidence:');
    if ((detail?.role_tags ?? []).length > 0) {
      lines.push(`  - role tags: \`${detail.role_tags.join(', ')}\``);
    }
    for (const [metric, value] of Object.entries(detail?.metrics ?? {})) {
      lines.push(`  - ${metric.replaceAll('_', ' ')}: \`${value}\``);
    }
    for (const evidence of detail?.evidence ?? []) {
      lines.push(`  - ${evidence}`);
    }
    appendCodeList(lines, 'candidate split axes', candidate.candidate_split_axes);
    appendRepoLinkList(lines, 'related surfaces', metadata.parallel_code_root, candidate.related_surfaces, 5);
    lines.push('');
  }

  lines.push('## Secondary Hotspots');
  lines.push('');
  for (const candidate of secondaryHotspots) {
    lines.push(`- \`${candidate.scope}\` ${candidate.summary}`);
  }
  if (secondaryHotspots.length === 0) {
    lines.push('- none');
  }
  lines.push('');

  lines.push('## Targeted Hardening Notes');
  lines.push('');
  for (const candidate of hardeningNotes) {
    lines.push(`- \`${candidate.scope}\` ${candidate.summary}`);
  }
  if (hardeningNotes.length === 0) {
    lines.push('- none');
  }
  lines.push('');

  lines.push('## Tooling Debt');
  lines.push('');
  for (const candidate of toolingDebt) {
    lines.push(`- ${formatRepoPathMarkdown(metadata.parallel_code_root, candidate.scope)} ${candidate.summary}`);
  }
  if (toolingDebt.length === 0) {
    lines.push('- none');
  }
  lines.push('');
  lines.push('## Top Watchpoints');
  lines.push('');
  for (const watchpoint of watchpoints) {
    lines.push(`### ${watchpoint.scope}`);
    lines.push('');
    lines.push(`- \`${watchpoint.trust_tier ?? 'watchpoint'}\``);
    lines.push(`- \`${watchpoint.kind}\``);
    lines.push(`- summary: \`${watchpoint.summary}\``);
    if (watchpoint.metrics?.length > 0) {
      lines.push('- evidence:');
      for (const metric of watchpoint.metrics) {
        if (metric.value === undefined || metric.value === null) {
          continue;
        }
        lines.push(`  - ${metric.label}: \`${metric.value}\``);
      }
    }
    if (watchpoint.cut_candidates?.length > 0) {
      lines.push('- candidate cuts:');
      for (const candidate of watchpoint.cut_candidates.slice(0, 3)) {
        lines.push(`  - \`${candidate.from} -> ${candidate.to}\``);
        lines.push(`    - seam kind: \`${candidate.seam_kind}\``);
        lines.push(`    - reduction: \`${candidate.reduction}\``);
      }
    }
    lines.push('');
  }
  lines.push('## Trusted Debt Clusters');
  lines.push('');
  for (const cluster of trustedClusters.slice(0, 5)) {
    lines.push(`### ${cluster.scope}`);
    lines.push('');
    lines.push(`- summary: \`${cluster.summary}\``);
    lines.push(`- trust tier: \`${cluster.trust_tier}\``);
    if (cluster.signal_kinds?.length > 0) {
      lines.push('- signal kinds:');
      for (const signalKind of cluster.signal_kinds) {
        lines.push(`  - \`${signalKind}\``);
      }
    }
    if (cluster.role_tags?.length > 0) {
      lines.push(`- role tags: \`${cluster.role_tags.join(', ')}\``);
    }
    lines.push('');
  }
  lines.push('## Experimental Side Channel');
  lines.push('');
  lines.push('Current experimental counts:');
  lines.push('');
  lines.push(`- experimental findings: \`${snapshot.experimental_findings.length}\``);
  lines.push(`- experimental debt signals: \`${experimentalSignals.length}\``);
  lines.push('');
  if (experimentalSignals.length > 0) {
    lines.push('Representative examples:');
    lines.push('');
    for (const signal of experimentalSignals.slice(0, 5)) {
      if (looksLikeRepoPath(signal.scope)) {
        lines.push(`- ${formatRepoPathMarkdown(metadata.parallel_code_root, signal.scope)}`);
      } else {
        lines.push(`- \`${signal.scope}\``);
      }
    }
    lines.push('');
  }
  lines.push('Current rule:');
  lines.push('');
  lines.push('- these are visible for analyzer follow-up');
  lines.push('- they should not be used as maintainer-facing debt guidance until the detector is fixed');
  lines.push('');
  lines.push('## Configured Concepts And Current State');
  lines.push('');
  for (const concept of snapshot.concept_summaries.slice(0, 5)) {
    lines.push(`### \`${concept.concept_id}\``);
    lines.push('');
    lines.push(`- score: \`${concept.score_0_10000 ?? 'n/a'} / 10000\``);
    lines.push(`- missing update sites: \`${concept.missing_site_count ?? 0}\``);
    lines.push(`- boundary pressure count: \`${concept.boundary_pressure_count ?? 0}\``);
    if ((concept.dominant_kinds ?? []).length > 0) {
      lines.push(`- dominant finding kinds: \`${concept.dominant_kinds.join(', ')}\``);
    }
    if (concept.summary) {
      lines.push(`- summary: ${concept.summary}`);
    }
    lines.push('');
  }

  return finalizeMarkdown(lines);
}

async function main() {
  const metadataPath = path.join(goldenDir, 'metadata.json');
  const scanPath = path.join(goldenDir, 'scan.json');
  const findingsPath = path.join(goldenDir, 'findings-top12.json');
  const obligationsPath = path.join(goldenDir, 'obligations-task_presentation_status.json');

  assertPathExists(goldenDir, 'parallel-code golden directory');
  assertPathExists(snapshotJsonPath, 'parallel-code proof snapshot JSON');
  assertPathExists(benchmarkPath, 'parallel-code benchmark artifact');
  assertPathExists(metadataPath, 'parallel-code metadata snapshot');
  assertPathExists(scanPath, 'parallel-code scan snapshot');
  assertPathExists(findingsPath, 'parallel-code findings snapshot');
  assertPathExists(obligationsPath, 'parallel-code obligations snapshot');

  const snapshot = readJson(snapshotJsonPath);
  const findings = readJson(findingsPath);
  const metadata = readJson(metadataPath);
  const scan = readJson(scanPath);
  const benchmark = readJson(benchmarkPath);
  const liveIdentity = collectRepoIdentity(parallelCodeRoot);
  const liveRulesIdentity = collectFileIdentity(metadata.rules_source);
  const liveBinaryIdentity = collectFileIdentity(metadata.sentrux_binary);

  if (!snapshotMatchesMetadata(snapshot, metadata)) {
    throw new Error(
      'parallel-code proof snapshot JSON is stale relative to the current goldens; regenerate the proof snapshot first',
    );
  }
  if (metadata.analysis_mode !== 'working_tree') {
    throw new Error(
      `parallel-code live report requires working_tree analysis metadata, got ${metadata.analysis_mode}`,
    );
  }

  assertRepoIdentityFresh({
    expected: metadata.source_tree_identity,
    actual: { ...liveIdentity, analysis_mode: metadata.analysis_mode },
    label: 'parallel-code goldens',
    allowStale: allowStaleGoldens,
  });
  assertRepoIdentityFresh({
    expected: metadata.analyzed_tree_identity,
    actual: { ...liveIdentity, analysis_mode: metadata.analysis_mode },
    label: 'parallel-code analyzed tree',
    allowStale: allowStaleGoldens,
  });
  assertFileIdentityFresh({
    expected: metadata.rules_identity,
    actual: liveRulesIdentity,
    label: 'parallel-code rules file',
    allowStale: allowStaleGoldens,
  });
  assertFileIdentityFresh({
    expected: metadata.binary_identity,
    actual: liveBinaryIdentity,
    label: 'parallel-code sentrux binary',
    allowStale: allowStaleGoldens,
  });

  await mkdir(path.dirname(snapshotMarkdownPath), { recursive: true });
  await writeFile(snapshotMarkdownPath, buildProofSnapshotMarkdown(snapshot), 'utf8');
  await writeFile(
    reportMarkdownPath,
    buildLiveEngineerReport({
      snapshot,
      findings,
      scan,
      benchmark,
      metadata,
      liveIdentity,
      allowStale: allowStaleGoldens,
    }),
    'utf8',
  );
  await writeFile(
    appendixMarkdownPath,
    buildLiveEngineerAppendix({
      snapshot,
      findings,
      scan,
      metadata,
    }),
    'utf8',
  );

  console.log(`Wrote proof snapshot Markdown to ${snapshotMarkdownPath}`);
  console.log(`Wrote live engineer report to ${reportMarkdownPath}`);
  console.log(`Wrote live engineer appendix to ${appendixMarkdownPath}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
