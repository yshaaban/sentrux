import path from 'node:path';

import { selectLeverageBuckets } from './v2-report-selection.mjs';

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

function looksLikeRepoPath(value) {
  if (typeof value !== 'string') {
    return false;
  }

  const repoPrefixes = ['src/', 'server/', 'electron/', 'scripts/', 'docs/'];
  return repoPrefixes.some((prefix) => value.startsWith(prefix));
}

function isSingleRepoPath(value) {
  return looksLikeRepoPath(value) && !value.includes('|');
}

function formatScopeHeading(repoPath, scope) {
  return isSingleRepoPath(scope) ? formatRepoPathMarkdown(repoPath, scope) : scope;
}

function formatScopeBullet(repoPath, scope) {
  return isSingleRepoPath(scope) ? formatRepoPathMarkdown(repoPath, scope) : `\`${scope}\``;
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
  lines.push(`### ${formatScopeHeading(repoRoot, candidate.scope)}`);
  lines.push('');
  appendCodeBullet(lines, 'trust tier', candidate.trust_tier ?? 'trusted');
  appendCodeBullet(lines, 'class', candidate.presentation_class ?? 'structural_debt');
  appendCodeBullet(lines, 'leverage', candidate.leverage_class ?? 'secondary_cleanup');
  appendCodeBullet(lines, 'signal band', candidate.score_band ?? 'supporting_signal');
  appendCodeBullet(lines, 'kind', candidate.kind ?? 'unknown');
  appendCodeBullet(lines, 'severity', candidate.severity ?? 'unknown');
  lines.push(`- summary: ${candidate.summary}`);
  if (candidate.impact) {
    lines.push(`- impact: ${candidate.impact}`);
  }
  appendCodeList(lines, 'leverage reasons', candidate.leverage_reasons);
  appendCodeList(lines, 'ranking reasons', candidate.ranking_reasons);
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

export function isHeadCloneAnalysis(metadata) {
  return metadata?.analysis_mode === 'head_clone';
}

export function snapshotMatchesMetadata(snapshot, metadata) {
  const snapshotMetadata = snapshot?.generated_from?.metadata;
  if (!snapshotMetadata) {
    return false;
  }

  return JSON.stringify(snapshotMetadata) === JSON.stringify(metadata);
}

export function assertHeadCommitFresh(metadata, liveIdentity, allowStale) {
  const expectedCommit = metadata?.source_tree_identity?.commit ?? null;
  const actualCommit = liveIdentity?.commit ?? null;

  if (expectedCommit === actualCommit || allowStale) {
    return;
  }

  throw new Error(
    `parallel-code HEAD commit changed: expected ${expectedCommit ?? 'unknown'}, got ${actualCommit ?? 'unknown'}`,
  );
}

function finalizeMarkdown(lines) {
  const output = [...lines];
  while (output.length > 0 && output.at(-1) === '') {
    output.pop();
  }

  return output.join('\n');
}

function buildReportHeading(headCloneAnalysis) {
  if (headCloneAnalysis) {
    return '# Parallel Code: Committed HEAD Analysis Report For Engineers';
  }

  return '# Parallel Code: Live Analysis Report For Engineers';
}

function buildAppendixHeading(headCloneAnalysis) {
  if (headCloneAnalysis) {
    return '# Parallel Code: Committed HEAD Analysis Report Appendix';
  }

  return '# Parallel Code: Live Analysis Report Appendix';
}

function buildAnalysisGeneratedLine(snapshot, metadata, headCloneAnalysis) {
  if (headCloneAnalysis) {
    return `Generated on ${formatUtcDate(snapshot.generated_at)} from a committed HEAD clone of \`${metadata.parallel_code_root}\`.`;
  }

  return `Generated on ${formatUtcDate(snapshot.generated_at)} from the live checkout at \`${metadata.parallel_code_root}\`.`;
}

function appendAnalysisHeader(lines, heading, generatedLine, audienceLines) {
  lines.push(heading);
  lines.push('');
  lines.push(generatedLine);
  lines.push('');
  for (const audienceLine of audienceLines) {
    lines.push(audienceLine);
  }
  lines.push('');
}

function appendFreshnessGateSection(lines, freshness, allowStale) {
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
}

function appendAnalyzedScopeSection(
  lines,
  { metadata, snapshotMarkdownPath, benchmarkPath, headCloneAnalysis, liveIdentity },
) {
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
  lines.push(`- comparison snapshot: \`${snapshotMarkdownPath}\``);
  lines.push(`- benchmark artifact: \`${benchmarkPath}\``);
  lines.push('');
}

function appendReportScanCoverageSection(lines, scan) {
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
}

function appendExecutiveSummarySection(lines, summaryCandidates) {
  lines.push('## Executive Summary');
  lines.push('');
  lines.push('The current analysis surfaces these highest-leverage improvement targets:');
  lines.push('');
  for (const candidate of summaryCandidates) {
    lines.push(
      `- \`${candidate.leverage_class}\` \`${candidate.score_band ?? 'supporting_signal'}\` \`${candidate.kind}\` ${candidate.summary}`,
    );
  }
  if (summaryCandidates.length === 0) {
    lines.push('- none');
  }
  lines.push('');
}

function appendDetailedCandidateSections(lines, repoRoot, sections) {
  for (const section of sections) {
    appendCandidateSection(lines, section.title, section.candidates, repoRoot);
  }
}

function appendWatchpointSummarySection(lines, watchpoints) {
  lines.push('## Watchpoints');
  lines.push('');
  for (const watchpoint of watchpoints) {
    lines.push(
      `- \`${watchpoint.trust_tier ?? 'watchpoint'}\` \`${watchpoint.leverage_class ?? 'secondary_cleanup'}\` \`${watchpoint.score_band ?? 'supporting_signal'}\` \`${watchpoint.kind}\` ${watchpoint.summary}`,
    );
  }
  if (watchpoints.length === 0) {
    lines.push('- none');
  }
  lines.push('');
}

function appendBenchmarkBaselineSection(lines, benchmark) {
  lines.push('## Benchmark Baseline');
  lines.push('');
  lines.push(`- cold process total: ${benchmark.benchmark?.cold_process_total_ms ?? 'n/a'} ms`);
  lines.push(`- warm cached total: ${benchmark.benchmark?.warm_cached_total_ms ?? 'n/a'} ms`);
  lines.push(
    `- warm patch-safety total: ${benchmark.benchmark?.warm_patch_safety_total_ms ?? 'n/a'} ms`,
  );
  lines.push('');
}

function appendFreshnessCheckResultSection(lines, liveIdentity, allowStale) {
  lines.push('## Freshness Check Result');
  lines.push('');
  lines.push(`- live commit: \`${liveIdentity.commit}\``);
  lines.push(`- live dirty paths: \`${liveIdentity.dirty_paths_count}\``);
  lines.push(`- live dirty-path fingerprint: \`${liveIdentity.dirty_paths_fingerprint}\``);
  lines.push(`- live tree fingerprint: \`${liveIdentity.tree_fingerprint}\``);
  if (allowStale) {
    lines.push('- freshness comparison: override enabled');
  } else {
    lines.push('- freshness comparison: goldens matched and report generation was allowed');
  }
  lines.push('');
}

function appendSourceDocumentsSection(lines, snapshotMarkdownPath, goldenDir) {
  lines.push('## Source Documents');
  lines.push('');
  lines.push(`- proof snapshot: \`${snapshotMarkdownPath}\``);
  lines.push(`- golden metadata: \`${path.join(goldenDir, 'metadata.json')}\``);
}

function appendAppendixMethodSection(lines, metadata, repoRoot, headCloneAnalysis) {
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
}

function appendAppendixScanCoverageSection(lines, scan) {
  lines.push('## Scan Scope And Confidence');
  lines.push('');
  lines.push('Current scan:');
  lines.push('');
  appendScanCoverage(lines, scan, { includeBuckets: true, includeSessionBaseline: true });
  lines.push('');
}

function buildFindingDetailKey(scope, kind) {
  return `${scope}\u0000${kind}`;
}

function buildFindingDetailMap(findings) {
  const detailMap = new Map();
  for (const detail of findings.finding_details ?? []) {
    detailMap.set(buildFindingDetailKey(detail.scope, detail.kind), detail);
  }
  return detailMap;
}

function appendCandidateEvidence(lines, detail) {
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
}

function appendLeadCandidateBlock(lines, candidate, detail, repoRoot) {
  lines.push(`### ${formatScopeHeading(repoRoot, candidate.scope)}`);
  lines.push('');
  lines.push(`- \`${candidate.trust_tier ?? 'trusted'}\``);
  lines.push(`- class: \`${candidate.presentation_class}\``);
  lines.push(`- leverage: \`${candidate.leverage_class}\``);
  lines.push(`- signal band: \`${candidate.score_band ?? 'supporting_signal'}\``);
  lines.push(`- \`${candidate.kind}\``);
  lines.push(`- summary: \`${candidate.summary}\``);
  lines.push(`- impact: ${candidate.impact}`);
  appendCodeList(lines, 'leverage reasons', candidate.leverage_reasons);
  appendCodeList(lines, 'ranking reasons', candidate.ranking_reasons);
  appendCandidateEvidence(lines, detail);
  appendCodeList(lines, 'candidate split axes', candidate.candidate_split_axes);
  appendRepoLinkList(lines, 'related surfaces', repoRoot, candidate.related_surfaces, 5);
  lines.push('');
}

function appendLeverageSummarySection(lines, leadCandidates, detailMap, repoRoot) {
  lines.push('## Leverage Summary');
  lines.push('');
  for (const candidate of leadCandidates) {
    const detail = detailMap.get(buildFindingDetailKey(candidate.scope, candidate.kind));
    appendLeadCandidateBlock(lines, candidate, detail, repoRoot);
  }
}

function appendCompactCandidateSection(lines, title, candidates) {
  lines.push(`## ${title}`);
  lines.push('');
  for (const candidate of candidates) {
    lines.push(
      `- \`${candidate.scope}\` \`${candidate.leverage_class}\` \`${candidate.score_band ?? 'supporting_signal'}\` ${candidate.summary}`,
    );
  }
  if (candidates.length === 0) {
    lines.push('- none');
  }
  lines.push('');
}

function appendHardeningNotesSection(lines, hardeningNotes) {
  lines.push('## Targeted Hardening Notes');
  lines.push('');
  for (const candidate of hardeningNotes) {
    lines.push(`- \`${candidate.scope}\` ${candidate.summary}`);
  }
  if (hardeningNotes.length === 0) {
    lines.push('- none');
  }
  lines.push('');
}

function appendToolingDebtSection(lines, toolingDebt, repoRoot) {
  lines.push('## Tooling Debt');
  lines.push('');
  for (const candidate of toolingDebt) {
    lines.push(`- ${formatScopeBullet(repoRoot, candidate.scope)} ${candidate.summary}`);
  }
  if (toolingDebt.length === 0) {
    lines.push('- none');
  }
  lines.push('');
}

function appendTopWatchpointsSection(lines, watchpoints) {
  lines.push('## Top Watchpoints');
  lines.push('');
  for (const watchpoint of watchpoints.slice(0, 6)) {
    lines.push(`### ${watchpoint.scope}`);
    lines.push('');
    lines.push(`- \`${watchpoint.trust_tier ?? 'watchpoint'}\``);
    lines.push(`- leverage: \`${watchpoint.leverage_class ?? 'secondary_cleanup'}\``);
    lines.push(`- signal band: \`${watchpoint.score_band ?? 'supporting_signal'}\``);
    lines.push(`- \`${watchpoint.kind}\``);
    lines.push(`- summary: \`${watchpoint.summary}\``);
    appendCodeList(lines, 'ranking reasons', watchpoint.ranking_reasons);
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
}

function appendTrustedDebtClustersSection(lines, trustedClusters) {
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
}

function appendExperimentalSideChannelSection(lines, snapshot, experimentalSignals, repoRoot) {
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
      lines.push(`- ${formatScopeBullet(repoRoot, signal.scope)}`);
    }
    lines.push('');
  }
  lines.push('Current rule:');
  lines.push('');
  lines.push('- these are visible for analyzer follow-up');
  lines.push('- they should not be used as maintainer-facing debt guidance until the detector is fixed');
  lines.push('');
}

function appendConceptSummariesSection(lines, conceptSummaries) {
  lines.push('## Configured Concepts And Current State');
  lines.push('');
  for (const concept of conceptSummaries.slice(0, 5)) {
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
}

export function buildLiveEngineerReport({
  snapshot,
  findings,
  scan,
  benchmark,
  metadata,
  liveIdentity,
  allowStale,
  snapshotMarkdownPath,
  goldenDir,
  benchmarkPath,
}) {
  const headCloneAnalysis = isHeadCloneAnalysis(metadata);
  const freshness = formatIdentity(metadata);
  const leverageBuckets = selectLeverageBuckets(findings);
  const lines = [];
  const reportSections = [
    { title: 'Architecture Signals', candidates: leverageBuckets.architecture_signals },
    { title: 'Best Local Refactor Targets', candidates: leverageBuckets.local_refactor_targets },
    { title: 'Boundary Discipline', candidates: leverageBuckets.boundary_discipline },
    { title: 'Regrowth Watchpoints', candidates: leverageBuckets.regrowth_watchpoints },
    { title: 'Secondary Cleanup', candidates: leverageBuckets.secondary_cleanup },
    { title: 'Targeted Hardening Notes', candidates: leverageBuckets.hardening_notes },
    { title: 'Tooling Debt', candidates: leverageBuckets.tooling_debt },
  ];

  appendAnalysisHeader(
    lines,
    buildReportHeading(headCloneAnalysis),
    buildAnalysisGeneratedLine(snapshot, metadata, headCloneAnalysis),
    ['This report is for an engineer who does not already know `parallel-code` or Sentrux.'],
  );
  appendFreshnessGateSection(lines, freshness, allowStale);
  appendAnalyzedScopeSection(lines, {
    metadata,
    snapshotMarkdownPath,
    benchmarkPath,
    headCloneAnalysis,
    liveIdentity,
  });
  appendReportScanCoverageSection(lines, scan);
  appendExecutiveSummarySection(lines, leverageBuckets.summary_candidates);
  appendDetailedCandidateSections(lines, metadata.parallel_code_root, reportSections);
  appendWatchpointSummarySection(lines, leverageBuckets.trusted_watchpoints);
  appendBenchmarkBaselineSection(lines, benchmark);
  appendFreshnessCheckResultSection(lines, liveIdentity, allowStale);
  appendSourceDocumentsSection(lines, snapshotMarkdownPath, goldenDir);

  return finalizeMarkdown(lines);
}

export function buildLiveEngineerAppendix({
  snapshot,
  findings,
  scan,
  metadata,
  reportMarkdownPath,
  repoRoot,
}) {
  const lines = [];
  const headCloneAnalysis = isHeadCloneAnalysis(metadata);
  const leverageBuckets = selectLeverageBuckets(findings);
  const trustedClusters = snapshot.debt_clusters.filter((cluster) => cluster.trust_tier === 'trusted');
  const experimentalSignals = findings.experimental_debt_signals ?? snapshot.experimental_debt_signals ?? [];
  const detailMap = buildFindingDetailMap(findings);
  const appendixSections = [
    { title: 'Architecture Signals', candidates: leverageBuckets.architecture_signals },
    { title: 'Best Local Refactor Targets', candidates: leverageBuckets.local_refactor_targets },
    { title: 'Boundary Discipline', candidates: leverageBuckets.boundary_discipline },
    { title: 'Regrowth Watchpoints', candidates: leverageBuckets.regrowth_watchpoints },
    { title: 'Secondary Cleanup', candidates: leverageBuckets.secondary_cleanup },
  ];

  appendAnalysisHeader(
    lines,
    buildAppendixHeading(headCloneAnalysis),
    buildAnalysisGeneratedLine(snapshot, metadata, headCloneAnalysis),
    [
      'This appendix contains the evidence behind',
      `[${path.basename(reportMarkdownPath)}](${reportMarkdownPath}).`,
    ],
  );
  appendAppendixMethodSection(lines, metadata, repoRoot, headCloneAnalysis);
  appendAppendixScanCoverageSection(lines, scan);
  appendLeverageSummarySection(
    lines,
    leverageBuckets.summary_candidates,
    detailMap,
    metadata.parallel_code_root,
  );
  for (const section of appendixSections) {
    appendCompactCandidateSection(lines, section.title, section.candidates);
  }
  appendHardeningNotesSection(lines, leverageBuckets.hardening_notes);
  appendToolingDebtSection(lines, leverageBuckets.tooling_debt, metadata.parallel_code_root);
  appendTopWatchpointsSection(lines, leverageBuckets.trusted_watchpoints);
  appendTrustedDebtClustersSection(lines, trustedClusters);
  appendExperimentalSideChannelSection(
    lines,
    snapshot,
    experimentalSignals,
    metadata.parallel_code_root,
  );
  appendConceptSummariesSection(lines, snapshot.concept_summaries);

  return finalizeMarkdown(lines);
}
