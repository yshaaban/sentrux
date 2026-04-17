import { collectDeadPrivateCandidateSets } from './dead-private.mjs';

function collectFindingKindCounts(findings) {
  const counts = {};

  for (const finding of findings ?? []) {
    const kind = finding?.kind ?? 'unknown';
    counts[kind] = (counts[kind] ?? 0) + 1;
  }

  return counts;
}

export function sanitizeRepoArtifactLabel(repoLabel) {
  const sanitized = String(repoLabel ?? 'repo')
    .trim()
    .replace(/[^a-zA-Z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toUpperCase();

  return sanitized || 'REPO';
}

function calculateRatio0To10000(numerator, denominator) {
  if (!Number.isFinite(numerator) || !Number.isFinite(denominator) || denominator <= 0) {
    return null;
  }

  return Math.round((numerator / denominator) * 10000);
}

function findDominantExclusionBucket(bucketedExclusions, totalExclusions = null) {
  const entries = Object.entries(bucketedExclusions ?? {}).filter(function hasFiniteValue([, value]) {
    return Number.isFinite(value);
  });
  if (entries.length === 0) {
    return {
      bucket: null,
      count: null,
      share_0_10000: null,
    };
  }

  const [bucket, count] = entries.reduce(function selectLargest(current, entry) {
    if (!current || entry[1] > current[1]) {
      return entry;
    }
    return current;
  }, null);

  return {
    bucket,
    count,
    share_0_10000: calculateRatio0To10000(
      count,
      Number.isFinite(totalExclusions)
        ? totalExclusions
        : entries.reduce(function sum(total, [, value]) {
            return total + value;
          }, 0),
    ),
  };
}

function shouldUseExclusionDrivenInterpretation({
  overallConfidence,
  totalExclusions,
  candidateFiles,
  keptFiles,
  dominantExclusionShare,
  internalResolutionConfidence,
}) {
  if (!Number.isFinite(overallConfidence) || overallConfidence >= 5000) {
    return false;
  }
  if (
    !Number.isFinite(totalExclusions) ||
    !Number.isFinite(candidateFiles) ||
    !Number.isFinite(keptFiles)
  ) {
    return false;
  }
  if (totalExclusions <= keptFiles) {
    return false;
  }
  if (!Number.isFinite(dominantExclusionShare) || dominantExclusionShare < 9000) {
    return false;
  }

  return (
    Number.isFinite(internalResolutionConfidence) && internalResolutionConfidence >= 9000
  );
}

function buildMixedRepoContext(scanTrust) {
  const candidateFiles = scanTrust.candidate_files ?? null;
  const keptFiles = scanTrust.kept_files ?? null;
  const trackedCandidates = scanTrust.tracked_candidates ?? null;
  const untrackedCandidates = scanTrust.untracked_candidates ?? null;
  const totalExclusions = scanTrust.exclusions?.total ?? null;
  const dominantExclusion = findDominantExclusionBucket(
    scanTrust.exclusions?.bucketed,
    totalExclusions,
  );
  const keptCandidateRatio = calculateRatio0To10000(keptFiles, candidateFiles);
  const excludedCandidateRatio = calculateRatio0To10000(totalExclusions, candidateFiles);
  const trackedCandidateRatio = calculateRatio0To10000(trackedCandidates, candidateFiles);
  const untrackedCandidateRatio = calculateRatio0To10000(untrackedCandidates, candidateFiles);
  const overallConfidence = scanTrust.overall_confidence_0_10000 ?? null;
  const internalResolutionConfidence = scanTrust.resolution?.internal_confidence_0_10000 ?? null;
  let interpretation =
    'Top-line scan confidence should be read alongside candidate exclusions and kept-file resolution.';

  if (
    shouldUseExclusionDrivenInterpretation({
      overallConfidence,
      totalExclusions,
      candidateFiles,
      keptFiles,
      dominantExclusionShare: dominantExclusion.share_0_10000,
      internalResolutionConfidence,
    })
  ) {
    interpretation =
      'Low top-line confidence is dominated by candidate exclusions in a mixed repo; the kept files still resolved internal imports cleanly.';
  }

  return {
    kept_candidate_ratio_0_10000: keptCandidateRatio,
    excluded_candidate_ratio_0_10000: excludedCandidateRatio,
    tracked_candidate_ratio_0_10000: trackedCandidateRatio,
    untracked_candidate_ratio_0_10000: untrackedCandidateRatio,
    dominant_exclusion_bucket: dominantExclusion.bucket,
    dominant_exclusion_count: dominantExclusion.count,
    dominant_exclusion_share_0_10000: dominantExclusion.share_0_10000,
    interpretation,
  };
}

export function buildScanCoverageBreakdown(rawToolAnalysis) {
  const scan = rawToolAnalysis.scan ?? {};
  const scanTrust = scan.scan_trust ?? {};
  const confidence = scan.confidence ?? {};

  return {
    repo_root: scan.scanned ?? null,
    note: 'Candidate-file coverage only; files filtered before candidate collection are not currently measured.',
    candidate_file_coverage: {
      mode: scanTrust.mode ?? null,
      candidate_files: scanTrust.candidate_files ?? null,
      tracked_candidates: scanTrust.tracked_candidates ?? null,
      untracked_candidates: scanTrust.untracked_candidates ?? null,
      kept_files: scanTrust.kept_files ?? null,
      scope_coverage_0_10000: scanTrust.scope_coverage_0_10000 ?? null,
      overall_confidence_0_10000: scanTrust.overall_confidence_0_10000 ?? null,
      partial: scanTrust.partial ?? null,
      truncated: scanTrust.truncated ?? null,
      fallback_reason: scanTrust.fallback_reason ?? null,
    },
    exclusions: {
      total: scanTrust.exclusions?.total ?? null,
      bucketed: {
        vendor: scanTrust.exclusions?.bucketed?.vendor ?? null,
        generated: scanTrust.exclusions?.bucketed?.generated ?? null,
        build: scanTrust.exclusions?.bucketed?.build ?? null,
        fixture: scanTrust.exclusions?.bucketed?.fixture ?? null,
        cache: scanTrust.exclusions?.bucketed?.cache ?? null,
      },
      ignored_extension: scanTrust.exclusions?.ignored_extension ?? null,
      too_large: scanTrust.exclusions?.too_large ?? null,
      metadata_error: scanTrust.exclusions?.metadata_error ?? null,
    },
    resolution: {
      resolved: scanTrust.resolution?.resolved ?? null,
      unresolved_internal: scanTrust.resolution?.unresolved_internal ?? null,
      unresolved_external: scanTrust.resolution?.unresolved_external ?? null,
      unresolved_unknown: scanTrust.resolution?.unresolved_unknown ?? null,
      internal_confidence_0_10000: scanTrust.resolution?.internal_confidence_0_10000 ?? null,
    },
    confidence: {
      scan_confidence_0_10000: confidence.scan_confidence_0_10000 ?? null,
      rule_coverage_0_10000: confidence.rule_coverage_0_10000 ?? null,
      semantic_rules_loaded: confidence.semantic_rules_loaded ?? null,
      session_baseline: confidence.session_baseline ?? null,
    },
    mixed_repo_context: buildMixedRepoContext(scanTrust),
  };
}

function formatCount(value) {
  return value ?? 'n/a';
}

function appendCodeBullet(lines, label, value) {
  lines.push(`- ${label}: \`${value}\``);
}

export function formatScanCoverageBreakdownMarkdown(breakdown) {
  const coverage = breakdown?.candidate_file_coverage ?? {};
  const exclusions = breakdown?.exclusions ?? {};
  const bucketedExclusions = exclusions.bucketed ?? {};
  const resolution = breakdown?.resolution ?? {};
  const confidence = breakdown?.confidence ?? {};
  const mixedRepoContext = breakdown?.mixed_repo_context ?? {};
  const lines = ['# Scan Coverage Breakdown', ''];

  appendCodeBullet(lines, 'repository analyzed', breakdown?.repo_root ?? 'unknown');
  lines.push(`- interpretation: ${breakdown?.note ?? 'n/a'}`);
  lines.push('');
  lines.push('## Candidate Coverage');
  lines.push('');
  appendCodeBullet(lines, 'scan mode', coverage.mode ?? 'unknown');
  lines.push(
    `- kept files: \`${formatCount(coverage.kept_files)} / ${formatCount(coverage.candidate_files)}\` candidate files`,
  );
  lines.push(`- tracked candidates: \`${formatCount(coverage.tracked_candidates)}\``);
  lines.push(`- untracked candidates: \`${formatCount(coverage.untracked_candidates)}\``);
  lines.push(`- scope coverage: \`${formatCount(coverage.scope_coverage_0_10000)} / 10000\``);
  lines.push(
    `- overall confidence: \`${formatCount(coverage.overall_confidence_0_10000)} / 10000\``,
  );
  lines.push(`- partial: \`${formatCount(coverage.partial)}\``);
  lines.push(`- truncated: \`${formatCount(coverage.truncated)}\``);
  lines.push(`- fallback reason: \`${formatCount(coverage.fallback_reason)}\``);
  lines.push('');
  lines.push('## Exclusions');
  lines.push('');
  lines.push(`- total measured exclusions: \`${formatCount(exclusions.total)}\``);
  lines.push(`- vendor: \`${formatCount(bucketedExclusions.vendor)}\``);
  lines.push(`- generated: \`${formatCount(bucketedExclusions.generated)}\``);
  lines.push(`- build: \`${formatCount(bucketedExclusions.build)}\``);
  lines.push(`- fixture: \`${formatCount(bucketedExclusions.fixture)}\``);
  lines.push(`- cache: \`${formatCount(bucketedExclusions.cache)}\``);
  lines.push(`- ignored extension: \`${formatCount(exclusions.ignored_extension)}\``);
  lines.push(`- too large: \`${formatCount(exclusions.too_large)}\``);
  lines.push(`- metadata error: \`${formatCount(exclusions.metadata_error)}\``);
  lines.push('');
  lines.push('## Mixed-Repo Context');
  lines.push('');
  lines.push(
    `- kept candidate ratio: \`${formatCount(mixedRepoContext.kept_candidate_ratio_0_10000)} / 10000\``,
  );
  lines.push(
    `- excluded candidate ratio: \`${formatCount(mixedRepoContext.excluded_candidate_ratio_0_10000)} / 10000\``,
  );
  lines.push(
    `- tracked candidate ratio: \`${formatCount(mixedRepoContext.tracked_candidate_ratio_0_10000)} / 10000\``,
  );
  lines.push(
    `- untracked candidate ratio: \`${formatCount(mixedRepoContext.untracked_candidate_ratio_0_10000)} / 10000\``,
  );
  lines.push(
    `- dominant exclusion bucket: \`${formatCount(mixedRepoContext.dominant_exclusion_bucket)}\``,
  );
  lines.push(
    `- dominant exclusion count: \`${formatCount(mixedRepoContext.dominant_exclusion_count)}\``,
  );
  lines.push(
    `- dominant exclusion share: \`${formatCount(mixedRepoContext.dominant_exclusion_share_0_10000)} / 10000\``,
  );
  lines.push(`- mixed-repo interpretation: ${mixedRepoContext.interpretation ?? 'n/a'}`);
  lines.push('');
  lines.push('## Resolution');
  lines.push('');
  lines.push(`- resolved imports: \`${formatCount(resolution.resolved)}\``);
  lines.push(`- unresolved internal: \`${formatCount(resolution.unresolved_internal)}\``);
  lines.push(`- unresolved external: \`${formatCount(resolution.unresolved_external)}\``);
  lines.push(`- unresolved unknown: \`${formatCount(resolution.unresolved_unknown)}\``);
  lines.push(
    `- internal resolution confidence: \`${formatCount(resolution.internal_confidence_0_10000)} / 10000\``,
  );
  lines.push('');
  lines.push('## Confidence');
  lines.push('');
  lines.push(`- scan confidence: \`${formatCount(confidence.scan_confidence_0_10000)} / 10000\``);
  lines.push(`- rule coverage: \`${formatCount(confidence.rule_coverage_0_10000)} / 10000\``);
  lines.push(`- semantic rules loaded: \`${formatCount(confidence.semantic_rules_loaded)}\``);
  if (confidence.session_baseline) {
    lines.push(
      `- session baseline: \`loaded=${formatCount(confidence.session_baseline.loaded)}, compatible=${formatCount(confidence.session_baseline.compatible)}, schema_version=${formatCount(confidence.session_baseline.schema_version)}\``,
    );
  }
  lines.push('');

  return `${lines.join('\n')}\n`;
}

export function buildRawToolSummary(rawToolAnalysis) {
  const findings = rawToolAnalysis.findings ?? {};
  const visibleFindings = findings.findings ?? [];
  const experimentalFindings = findings.experimental_findings ?? [];
  const experimentalDebtSignals = findings.experimental_debt_signals ?? [];
  const deadPrivateCandidates = collectDeadPrivateCandidateSets(rawToolAnalysis);
  const scanCoverageBreakdown = buildScanCoverageBreakdown(rawToolAnalysis);
  const scanSummary = scanCoverageBreakdown.candidate_file_coverage;
  const scanResolution = scanCoverageBreakdown.resolution;
  const scanConfidence = scanCoverageBreakdown.confidence;
  const mixedRepoContext = scanCoverageBreakdown.mixed_repo_context;

  return {
    repo_root: rawToolAnalysis.scan?.scanned ?? null,
    scan_summary: {
      lines: rawToolAnalysis.scan?.lines ?? null,
      quality_signal: rawToolAnalysis.scan?.quality_signal ?? null,
      mode: scanSummary.mode,
      kept_files: scanSummary.kept_files,
      candidate_files: scanSummary.candidate_files,
      tracked_candidates: scanSummary.tracked_candidates,
      untracked_candidates: scanSummary.untracked_candidates,
      scope_coverage_0_10000: scanSummary.scope_coverage_0_10000,
      overall_confidence_0_10000: scanSummary.overall_confidence_0_10000,
      partial: scanSummary.partial,
      truncated: scanSummary.truncated,
      fallback_reason: scanSummary.fallback_reason,
      scan_confidence_0_10000: scanConfidence.scan_confidence_0_10000,
      rule_coverage_0_10000: scanConfidence.rule_coverage_0_10000,
      semantic_rules_loaded: scanConfidence.semantic_rules_loaded,
      exclusions: scanCoverageBreakdown.exclusions,
      resolution: scanResolution,
      unresolved_internal: scanResolution.unresolved_internal,
      unresolved_external: scanResolution.unresolved_external,
      unresolved_unknown: scanResolution.unresolved_unknown,
      mixed_repo_context: mixedRepoContext,
    },
    check_summary: {
      gate: rawToolAnalysis.check?.gate ?? null,
      summary: rawToolAnalysis.check?.summary ?? null,
      action_count: (rawToolAnalysis.check?.actions ?? []).length,
      issue_count: (rawToolAnalysis.check?.issues ?? []).length,
    },
    gate_summary: {
      decision: rawToolAnalysis.gate?.decision ?? null,
      summary: rawToolAnalysis.gate?.summary ?? null,
      blocking_count: (rawToolAnalysis.gate?.blocking_findings ?? []).length,
      introduced_count: (rawToolAnalysis.gate?.introduced_findings ?? []).length,
      obligation_completeness_0_10000:
        rawToolAnalysis.gate?.obligation_completeness_0_10000 ?? null,
    },
    findings_summary: {
      findings_count: visibleFindings.length,
      watchpoint_count: (findings.watchpoints ?? []).length,
      experimental_finding_count: experimentalFindings.length,
      experimental_debt_signal_count: experimentalDebtSignals.length,
      dead_private_source_lane: deadPrivateCandidates.sourceLane,
      dead_private_source_lane_count: deadPrivateCandidates.sourceLaneCount,
      dead_private_lane_considered: deadPrivateCandidates.consideredLanes,
      dead_private_reviewer_lane_status: deadPrivateCandidates.reviewerLaneStatus,
      dead_private_reviewer_lane_reason: deadPrivateCandidates.reviewerLaneReason,
      dead_private_canonical_candidate_count: deadPrivateCandidates.canonicalCandidateCount,
      dead_private_legacy_candidate_count: deadPrivateCandidates.legacyCandidateCount,
      dead_private_overlap_count: deadPrivateCandidates.overlappingCandidateCount,
      dead_private_candidate_count: deadPrivateCandidates.selectedCandidates.length,
      dead_private_legacy_only_count: deadPrivateCandidates.legacyOnlyCandidates.length,
      kind_counts: {
        ...collectFindingKindCounts(visibleFindings),
        experimental_dead_private_code_cluster: deadPrivateCandidates.combinedCandidates.length,
      },
    },
    session_end_summary: {
      pass: rawToolAnalysis.session_end?.pass ?? null,
      summary: rawToolAnalysis.session_end?.summary ?? null,
      action_count: (rawToolAnalysis.session_end?.actions ?? []).length,
      introduced_findings_count: (rawToolAnalysis.session_end?.introduced_findings ?? []).length,
      signal_before: rawToolAnalysis.session_end?.signal_before ?? null,
      signal_after: rawToolAnalysis.session_end?.signal_after ?? null,
      signal_delta: rawToolAnalysis.session_end?.signal_delta ?? null,
    },
  };
}
