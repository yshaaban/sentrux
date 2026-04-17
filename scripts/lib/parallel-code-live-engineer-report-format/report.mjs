import { selectLeverageBuckets } from '../v2-report-selection.mjs';
import { isHeadCloneAnalysis } from './identity.mjs';
import {
  appendAnalysisHeader,
  appendAnalyzedScopeSection,
  appendCodeBullet,
  appendCodeList,
  appendFreshnessGateSection,
  appendRepoLinkList,
  appendSourceDocumentsSection,
  buildAnalysisGeneratedLine,
  buildReportHeading,
  finalizeMarkdown,
  formatIdentity,
  formatScopeHeading,
} from './common.mjs';

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
