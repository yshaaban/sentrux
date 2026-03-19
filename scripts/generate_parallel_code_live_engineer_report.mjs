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
import { selectLeverageBuckets } from './lib/v2-report-selection.mjs';

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

function isHeadCloneAnalysis(metadata) {
  return metadata?.analysis_mode === 'head_clone';
}

function assertHeadCommitFresh(metadata, liveIdentity, allowStale) {
  const expectedCommit = metadata?.source_tree_identity?.commit ?? null;
  const actualCommit = liveIdentity?.commit ?? null;

  if (expectedCommit === actualCommit || allowStale) {
    return;
  }

  throw new Error(
    `parallel-code HEAD commit changed: expected ${expectedCommit ?? 'unknown'}, got ${actualCommit ?? 'unknown'}`,
  );
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
  appendCodeBullet(lines, 'trust tier', candidate.trust_tier ?? 'trusted');
  appendCodeBullet(lines, 'class', candidate.presentation_class ?? 'structural_debt');
  appendCodeBullet(lines, 'leverage', candidate.leverage_class ?? 'secondary_cleanup');
  appendCodeBullet(lines, 'kind', candidate.kind ?? 'unknown');
  appendCodeBullet(lines, 'severity', candidate.severity ?? 'unknown');
  lines.push(`- summary: ${candidate.summary}`);
  if (candidate.impact) {
    lines.push(`- impact: ${candidate.impact}`);
  }
  appendCodeList(lines, 'leverage reasons', candidate.leverage_reasons);
  appendCodeList(lines, 'candidate split axes', candidate.candidate_split_axes);
  appendRepoLinkList(lines, 'related surfaces', repoRoot, candidate.related_surfaces, 5);
  lines.push('');
}

function appendCandidateSection(lines, title, candidates, repoRoot) {
  lines.push(`## ${title}`);
  lines.push('');

  for (const candidate of candidates) {
    appendCandidateBlock(lines, candidate, repoRoot);
  }

  if (candidates.length === 0) {
    lines.push('- none');
    lines.push('');
  }
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

function buildLiveEngineerReport({
  snapshot,
  findings,
  scan,
  benchmark,
  metadata,
  liveIdentity,
  allowStale,
}) {
  const headCloneAnalysis = isHeadCloneAnalysis(metadata);
  const freshness = formatIdentity(metadata);
  const leverageBuckets = selectLeverageBuckets(findings);
  const summaryCandidates = leverageBuckets.summary_candidates;
  const architectureSignals = leverageBuckets.architecture_signals;
  const localRefactorTargets = leverageBuckets.local_refactor_targets;
  const boundaryDiscipline = leverageBuckets.boundary_discipline;
  const regrowthWatchpoints = leverageBuckets.regrowth_watchpoints;
  const secondaryCleanup = leverageBuckets.secondary_cleanup;
  const hardeningNotes = leverageBuckets.hardening_notes;
  const toolingDebt = leverageBuckets.tooling_debt;
  const lines = [];
  lines.push(
    headCloneAnalysis
      ? '# Parallel Code: Committed HEAD Analysis Report For Engineers'
      : '# Parallel Code: Live Analysis Report For Engineers',
  );
  lines.push('');
  lines.push(
    headCloneAnalysis
      ? `Generated on ${formatUtcDate(snapshot.generated_at)} from a committed HEAD clone of \`${metadata.parallel_code_root}\`.`
      : `Generated on ${formatUtcDate(snapshot.generated_at)} from the live checkout at \`${metadata.parallel_code_root}\`.`,
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
  if (headCloneAnalysis) {
    lines.push('- report scope: committed `HEAD` only');
    lines.push(
      `- ignored working-tree changes outside HEAD: \`${liveIdentity?.dirty_paths?.length ?? 0}\``,
    );
  }
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
  lines.push('The current analysis surfaces these highest-leverage improvement targets:');
  lines.push('');
  for (const candidate of summaryCandidates) {
    lines.push(
      `- \`${candidate.leverage_class}\` \`${candidate.kind}\` ${candidate.summary}`,
    );
  }
  if (summaryCandidates.length === 0) {
    lines.push('- none');
  }
  lines.push('');
  appendCandidateSection(lines, 'Architecture Signals', architectureSignals, metadata.parallel_code_root);
  appendCandidateSection(
    lines,
    'Best Local Refactor Targets',
    localRefactorTargets,
    metadata.parallel_code_root,
  );
  appendCandidateSection(
    lines,
    'Boundary Discipline',
    boundaryDiscipline,
    metadata.parallel_code_root,
  );
  appendCandidateSection(
    lines,
    'Regrowth Watchpoints',
    regrowthWatchpoints,
    metadata.parallel_code_root,
  );
  appendCandidateSection(
    lines,
    'Secondary Cleanup',
    secondaryCleanup,
    metadata.parallel_code_root,
  );
  appendCandidateSection(
    lines,
    'Targeted Hardening Notes',
    hardeningNotes,
    metadata.parallel_code_root,
  );
  appendCandidateSection(lines, 'Tooling Debt', toolingDebt, metadata.parallel_code_root);

  lines.push('## Watchpoints');
  lines.push('');
  const watchpoints = leverageBuckets.trusted_watchpoints;
  for (const watchpoint of watchpoints) {
    lines.push(
      `- \`${watchpoint.trust_tier ?? 'watchpoint'}\` \`${watchpoint.leverage_class ?? 'secondary_cleanup'}\` \`${watchpoint.kind}\` ${watchpoint.summary}`,
    );
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
  const headCloneAnalysis = isHeadCloneAnalysis(metadata);
  const leverageBuckets = selectLeverageBuckets(findings);
  const leadCandidates = leverageBuckets.summary_candidates;
  const architectureSignals = leverageBuckets.architecture_signals;
  const localRefactorTargets = leverageBuckets.local_refactor_targets;
  const boundaryDiscipline = leverageBuckets.boundary_discipline;
  const regrowthWatchpoints = leverageBuckets.regrowth_watchpoints;
  const secondaryCleanup = leverageBuckets.secondary_cleanup;
  const hardeningNotes = leverageBuckets.hardening_notes;
  const toolingDebt = leverageBuckets.tooling_debt;
  const watchpoints = leverageBuckets.trusted_watchpoints;
  const trustedClusters = snapshot.debt_clusters.filter((cluster) => cluster.trust_tier === 'trusted');
  const experimentalSignals = findings.experimental_debt_signals ?? snapshot.experimental_debt_signals ?? [];

  lines.push(
    headCloneAnalysis
      ? '# Parallel Code: Committed HEAD Analysis Report Appendix'
      : '# Parallel Code: Live Analysis Report Appendix',
  );
  lines.push('');
  lines.push(
    headCloneAnalysis
      ? `Generated on ${formatUtcDate(snapshot.generated_at)} from a committed HEAD clone of \`${metadata.parallel_code_root}\`.`
      : `Generated on ${formatUtcDate(snapshot.generated_at)} from the live checkout at \`${metadata.parallel_code_root}\`.`,
  );
  lines.push('');
  lines.push('This appendix contains the evidence behind');
  lines.push(`[${path.basename(reportMarkdownPath)}](${reportMarkdownPath}).`);
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
  if (headCloneAnalysis) {
    lines.push('- this report intentionally ignores uncommitted working-tree changes');
  }
  lines.push('');
  lines.push('## Scan Scope And Confidence');
  lines.push('');
  lines.push('Current scan:');
  lines.push('');
  appendScanCoverage(lines, scan, { includeBuckets: true, includeSessionBaseline: true });
  lines.push('');
  lines.push('## Leverage Summary');
  lines.push('');
  for (const candidate of leadCandidates) {
    if (looksLikeRepoPath(candidate.scope)) {
      lines.push(`### ${formatRepoPathMarkdown(metadata.parallel_code_root, candidate.scope)}`);
    } else {
      lines.push(`### ${candidate.scope}`);
    }
    lines.push('');
    lines.push(`- \`${candidate.trust_tier ?? 'trusted'}\``);
    lines.push(`- class: \`${candidate.presentation_class}\``);
    lines.push(`- leverage: \`${candidate.leverage_class}\``);
    lines.push(`- \`${candidate.kind}\``);
    lines.push(`- summary: \`${candidate.summary}\``);
    lines.push(`- impact: ${candidate.impact}`);
    appendCodeList(lines, 'leverage reasons', candidate.leverage_reasons);
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

  for (const [title, candidates] of [
    ['Architecture Signals', architectureSignals],
    ['Best Local Refactor Targets', localRefactorTargets],
    ['Boundary Discipline', boundaryDiscipline],
    ['Regrowth Watchpoints', regrowthWatchpoints],
    ['Secondary Cleanup', secondaryCleanup],
  ]) {
    lines.push(`## ${title}`);
    lines.push('');
    for (const candidate of candidates) {
      lines.push(
        `- \`${candidate.scope}\` \`${candidate.leverage_class}\` ${candidate.summary}`,
      );
    }
    if (candidates.length === 0) {
      lines.push('- none');
    }
    lines.push('');
  }

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
  for (const watchpoint of watchpoints.slice(0, 6)) {
    lines.push(`### ${watchpoint.scope}`);
    lines.push('');
    lines.push(`- \`${watchpoint.trust_tier ?? 'watchpoint'}\``);
    lines.push(`- leverage: \`${watchpoint.leverage_class ?? 'secondary_cleanup'}\``);
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
  if (!['working_tree', 'head_clone'].includes(metadata.analysis_mode)) {
    throw new Error(
      `parallel-code report requires working_tree or head_clone analysis metadata, got ${metadata.analysis_mode}`,
    );
  }

  if (metadata.analysis_mode === 'working_tree') {
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
  } else {
    assertHeadCommitFresh(metadata, liveIdentity, allowStaleGoldens);
  }
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

  console.log(`Wrote live engineer report to ${reportMarkdownPath}`);
  console.log(`Wrote live engineer appendix to ${appendixMarkdownPath}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
