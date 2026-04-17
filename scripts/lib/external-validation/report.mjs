import {
  appendDeadPrivateCandidateSection,
  collectDeadPrivateCandidateSets,
  deadPrivateFalsePositiveCandidates,
  deadPrivatePlausibleCandidates,
  sortByNumericField,
} from './dead-private.mjs';

function topLargeFiles(rawToolAnalysis, limit) {
  return (rawToolAnalysis.findings?.findings ?? [])
    .filter(function isLargeFile(finding) {
      return finding.kind === 'large_file';
    })
    .slice(0, limit);
}

function topCycles(rawToolAnalysis, limit) {
  return (rawToolAnalysis.findings?.findings ?? [])
    .filter(function isCycle(finding) {
      return finding.kind === 'cycle_cluster';
    })
    .slice(0, limit);
}

function topClones(rawToolAnalysis, limit) {
  return sortByNumericField(
    (rawToolAnalysis.findings?.findings ?? []).filter(function isClone(finding) {
      return finding.kind === 'exact_clone_group';
    }),
    'total_lines',
  ).slice(0, limit);
}

function appendCodeBullet(lines, label, value) {
  lines.push(`- ${label}: \`${value}\``);
}

export function buildPacketValidation(packet) {
  const samples = Array.isArray(packet?.samples) ? packet.samples : [];
  const cloneSamples = samples.filter(function isCloneSample(sample) {
    return sample?.kind === 'exact_clone_group';
  });
  const richCloneSamples = cloneSamples.filter(function hasRichCloneEvidence(sample) {
    return (
      Array.isArray(sample?.clone_evidence?.files) &&
      sample.clone_evidence.files.length > 0 &&
      Array.isArray(sample?.clone_evidence?.instances) &&
      sample.clone_evidence.instances.length > 0 &&
      Array.isArray(sample?.clone_evidence?.recent_edit_reasons) &&
      sample.clone_evidence.recent_edit_reasons.length > 0
    );
  });

  return {
    sample_count: samples.length,
    clone_sample_count: cloneSamples.length,
    rich_clone_sample_count: richCloneSamples.length,
    surfaces_scan_confidence: Number.isFinite(
      packet?.scan_metadata?.confidence?.scan_confidence_0_10000,
    ),
    surfaces_rule_coverage: Number.isFinite(
      packet?.scan_metadata?.confidence?.rule_coverage_0_10000,
    ),
  };
}

export function buildValidationReport({
  repoRootPath,
  repoLabel,
  branch,
  commit,
  workingTreeClean,
  rawToolAnalysis,
  rawToolSummary,
  packetValidation = null,
  scanCoverageBreakdown = null,
}) {
  const largeFiles = topLargeFiles(rawToolAnalysis, 3);
  const cycles = topCycles(rawToolAnalysis, 2);
  const clones = topClones(rawToolAnalysis, 5);
  const deadPrivateFalsePositives = deadPrivateFalsePositiveCandidates(rawToolAnalysis);
  const scanSummary = rawToolSummary.scan_summary ?? {};
  const findingsSummary = rawToolSummary.findings_summary ?? {};
  const mixedRepoContext =
    scanSummary.mixed_repo_context ?? scanCoverageBreakdown?.mixed_repo_context ?? {};
  const lines = [];

  appendValidationScope(lines, { repoRootPath, repoLabel, branch, commit, workingTreeClean });
  appendValidationStrengths(lines, {
    rawToolSummary,
    findingsSummary,
    largeFiles,
    cycles,
    packetValidation,
    scanCoverageBreakdown,
  });
  appendValidationImprovements(lines, {
    repoLabel,
    packetValidation,
    scanSummary,
    findingsSummary,
    clones,
    deadPrivateFalsePositives,
    mixedRepoContext,
  });
  appendValidationNextSteps(lines, {
    packetValidation,
    clones,
    findingsSummary,
    scanCoverageBreakdown,
  });
  appendValidationBottomLine(lines, { repoLabel, packetValidation });

  return `${lines.join('\n')}\n`;
}

function appendValidationScope(lines, { repoRootPath, repoLabel, branch, commit, workingTreeClean }) {
  lines.push(`# ${repoLabel} Metrics Validation Report`);
  lines.push('');
  lines.push('## Scope');
  lines.push('');
  appendCodeBullet(lines, 'repository analyzed', repoRootPath);
  appendCodeBullet(lines, 'branch', branch ?? 'unknown');
  appendCodeBullet(lines, 'commit', commit ?? 'unknown');
  appendCodeBullet(lines, 'working tree', workingTreeClean ? 'clean' : 'dirty');
  lines.push('- goal: validate Sentrux metrics and reviewer-facing outputs against an external repo');
  lines.push('');
}

function appendValidationStrengths(
  lines,
  { rawToolSummary, findingsSummary, largeFiles, cycles, packetValidation, scanCoverageBreakdown },
) {
  lines.push('## What Validated Well');
  lines.push('');
  lines.push(
    `- clean-repo gating stayed quiet: check=${rawToolSummary.check_summary.gate ?? 'unknown'}, gate=${rawToolSummary.gate_summary.decision ?? 'unknown'}, session_end=${rawToolSummary.session_end_summary.pass ? 'pass' : 'non-pass'}`,
  );
  if (largeFiles.length > 0) {
    lines.push(
      `- large-file findings were concrete: ${largeFiles.map(function formatFinding(finding) {
        return `${finding.scope} (${finding.metrics?.line_count ?? 'n/a'} lines)`;
      }).join(', ')}`,
    );
  }
  if (cycles.length > 0) {
    lines.push(
      `- cycle findings were actionable: ${cycles.map(function formatCycle(finding) {
        return `${finding.scope} (${finding.metrics?.cycle_size ?? 'n/a'} files)`;
      }).join(', ')}`,
    );
  }
  if (findingsSummary.kind_counts?.exact_clone_group) {
    lines.push(
      `- clone detection found a real maintenance pattern: ${findingsSummary.kind_counts.exact_clone_group} exact clone groups across example/template surfaces`,
    );
  }
  if (packetValidation?.rich_clone_sample_count > 0) {
    lines.push(
      `- clone review packets now preserve concrete evidence for ${packetValidation.rich_clone_sample_count} sampled clone findings, including file paths, line counts, and recent-edit reasons`,
    );
  }
  if (packetValidation?.surfaces_scan_confidence && packetValidation?.surfaces_rule_coverage) {
    lines.push('- review packets now surface scan confidence and rule coverage in the first screen');
  }
  if (scanCoverageBreakdown) {
    lines.push(
      '- the scan coverage breakdown artifact now preserves candidate coverage, exclusion buckets, fallback state, and resolution counts for the run',
    );
  }
  if (findingsSummary.dead_private_reviewer_lane_status) {
    lines.push(
      `- dead-private review routing is explicit: reviewer queue=\`${findingsSummary.dead_private_source_lane ?? 'none'}\`, status=\`${findingsSummary.dead_private_reviewer_lane_status}\`, queued=\`${findingsSummary.dead_private_candidate_count ?? 0}\`, legacy-only watchlist=\`${findingsSummary.dead_private_legacy_only_count ?? 0}\``,
    );
  }
  lines.push('');
}

function appendValidationImprovements(
  lines,
  {
    repoLabel,
    packetValidation,
    scanSummary,
    findingsSummary,
    clones,
    deadPrivateFalsePositives,
    mixedRepoContext,
  },
) {
  lines.push('## What Needs Improvement');
  lines.push('');
  if (deadPrivateFalsePositives.length > 0) {
    lines.push(
      `- dead-private precision is not good enough yet; ${repoLabel} exposed false positives from repeated callback-style helper names and similar low-confidence samples:`,
    );
    appendDeadPrivateCandidateSection(lines, deadPrivateFalsePositives.slice(0, 5), 'sample symbols');
  } else {
    lines.push('- dead-private precision still needs broader external validation');
  }
  if (clones.length > 0 && !packetValidation?.rich_clone_sample_count) {
    lines.push(
      '- clone packet output is too lossy compared to the raw payload; the current packet path needs to preserve file paths, clone sizes, and drift reasons',
    );
  }
  if (packetValidation?.surfaces_scan_confidence) {
    const lowConfidenceLine = [
      `- ${repoLabel} still scans with low confidence: only ${scanSummary.kept_files ?? 'n/a'} of ${scanSummary.candidate_files ?? 'n/a'} candidate files were kept, and overall confidence is ${scanSummary.overall_confidence_0_10000 ?? 'n/a'} / 10000.`,
    ];
    if (Number.isFinite(scanSummary.exclusions?.total) && mixedRepoContext.dominant_exclusion_bucket) {
      lowConfidenceLine.push(
        `${scanSummary.exclusions.total} candidates were excluded before deep analysis, dominated by ${mixedRepoContext.dominant_exclusion_bucket} exclusions (${mixedRepoContext.dominant_exclusion_count ?? 'n/a'} files, ${mixedRepoContext.dominant_exclusion_share_0_10000 ?? 'n/a'} / 10000 of measured exclusions), while kept-file internal resolution stayed ${scanSummary.resolution?.internal_confidence_0_10000 ?? 'n/a'} / 10000.`,
      );
    }
    lines.push(lowConfidenceLine.join(' '));
  } else {
    lines.push(
      `- scan trust must be more visible: only ${scanSummary.kept_files ?? 'n/a'} of ${scanSummary.candidate_files ?? 'n/a'} candidate files were kept, with confidence ${scanSummary.overall_confidence_0_10000 ?? 'n/a'} / 10000`,
    );
  }
  if (findingsSummary.dead_private_legacy_only_count > 0) {
    lines.push(
      `- dead-private taxonomy still needs cleanup: ${findingsSummary.dead_private_legacy_only_count} legacy-only candidate(s) remain outside the canonical reviewer queue even though the reviewer routing is now explicit`,
    );
  }
  lines.push('');
}

function appendValidationNextSteps(
  lines,
  { packetValidation, clones, findingsSummary, scanCoverageBreakdown },
) {
  lines.push('## Highest-ROI Next Steps');
  lines.push('');
  lines.push(
    '- tighten dead-private classification and measure precision against the exported external false-positive set',
  );
  if (clones.length > 0 && !packetValidation?.rich_clone_sample_count) {
    lines.push('- enrich clone review packets with file paths, line counts, and recent-edit asymmetry reasons');
  }
  if (packetValidation?.surfaces_scan_confidence) {
    lines.push(
      '- improve eligible coverage reporting and candidate retention on large mixed repos without hiding exclusion-driven pressure',
    );
  } else {
    lines.push('- surface scan trust and coverage in the first screen of every review surface');
  }
  if (scanCoverageBreakdown) {
    lines.push('- use the scan coverage breakdown artifact to separate precision issues from candidate-coverage losses');
  }
  if (findingsSummary.dead_private_legacy_only_count > 0) {
    lines.push('- unify or retire the legacy dead-private watchlist so reviewer routing and remediation queues match');
  }
  lines.push('');
}

function appendValidationBottomLine(lines, { repoLabel, packetValidation }) {
  lines.push('## Bottom Line');
  lines.push('');
  if (packetValidation?.rich_clone_sample_count) {
    lines.push(
      `${repoLabel} confirmed that Sentrux is already useful for clean-repo gating, duplicate-drift detection, and reviewer-facing evidence packaging. The main remaining trust gaps are dead-private precision calibration and low scan confidence on large mixed repos.`,
    );
  } else {
    lines.push(
      `${repoLabel} confirmed that Sentrux is already useful for clean-repo gating and duplicate-drift detection. The main trust breakers are dead-private precision and evidence loss in the clone packet path.`,
    );
  }
  lines.push('');
}

function appendPrioritySection(lines, title, bullets) {
  lines.push(`## ${title}`);
  lines.push('');
  for (const bullet of bullets) {
    lines.push(`- ${bullet}`);
  }
  lines.push('');
}

export function buildEngineeringReport({
  repoRootPath,
  repoLabel,
  branch,
  commit,
  rawToolAnalysis,
}) {
  const largeFiles = topLargeFiles(rawToolAnalysis, 3);
  const cycles = topCycles(rawToolAnalysis, 2);
  const clones = topClones(rawToolAnalysis, 10);
  const deadPrivateCandidateSets = collectDeadPrivateCandidateSets(rawToolAnalysis);
  const plausibleDeadPrivate = deadPrivatePlausibleCandidates(rawToolAnalysis).slice(0, 5);
  const skepticalDeadPrivate = deadPrivateFalsePositiveCandidates(rawToolAnalysis).slice(0, 5);
  const lines = [];

  appendEngineeringScope(lines, { repoRootPath, repoLabel, branch, commit });
  appendEngineeringCycles(lines, cycles);
  appendEngineeringCloneDrift(lines, clones);
  appendEngineeringLargeFiles(lines, largeFiles);
  appendEngineeringDeadPrivate(lines, {
    deadPrivateCandidateSets,
    plausibleDeadPrivate,
    skepticalDeadPrivate,
  });
  appendEngineeringBottomLine(lines);

  return `${lines.join('\n')}\n`;
}

function appendEngineeringScope(lines, { repoRootPath, repoLabel, branch, commit }) {
  lines.push(`# ${repoLabel} Engineering Report`);
  lines.push('');
  lines.push('## Scope');
  lines.push('');
  appendCodeBullet(lines, 'repository analyzed', repoRootPath);
  appendCodeBullet(lines, 'branch', branch ?? 'unknown');
  appendCodeBullet(lines, 'commit', commit ?? 'unknown');
  lines.push('- analysis mode: static repository-wide structural analysis');
  lines.push('- no runtime verification or behavior tests were executed as part of this report');
  lines.push('');
  appendPrioritySection(lines, 'Executive Summary', [
    'High-confidence work: break dependency cycles, reduce template/example duplication drift, and split the largest responsibility-heavy files.',
    'Lower-confidence work: audit dead-private candidates manually instead of applying automated cleanup blindly.',
  ]);
}

function appendEngineeringCycles(lines, cycles) {
  lines.push('## Priority 1: Break The Dependency Cycles');
  lines.push('');
  for (const finding of cycles) {
    lines.push(`### \`${finding.scope}\``);
    lines.push('');
    lines.push(`- summary: ${finding.summary}`);
    lines.push(`- impact: ${finding.impact}`);
    lines.push(
      `- best cut: ${
        finding.cut_candidates?.[0]?.summary ??
        'inspect the candidate back-edge and split contracts from implementations'
      }`,
    );
    lines.push('');
  }
  if (cycles.length === 0) {
    lines.push('- none');
    lines.push('');
  }
}

function appendEngineeringCloneDrift(lines, clones) {
  lines.push('## Priority 1: Reduce Template And Example Duplication Drift');
  lines.push('');
  for (const finding of clones.slice(0, 5)) {
    lines.push(`- \`${(finding.files ?? []).join(' | ')}\``);
    lines.push(`  - total cloned lines: \`${finding.total_lines ?? 'n/a'}\``);
    lines.push(`  - drift reasons: \`${(finding.reasons ?? []).join('; ')}\``);
  }
  if (clones.length === 0) {
    lines.push('- none');
  }
  lines.push('');
  lines.push('- recommendation: pick one canonical source for shared example logic and enforce sync mechanically');
  lines.push('');
}

function appendEngineeringLargeFiles(lines, largeFiles) {
  lines.push('## Priority 1: Split The Largest Responsibility-Heavy Files');
  lines.push('');
  for (const finding of largeFiles) {
    lines.push(`- \`${finding.scope}\``);
    lines.push(`  - line count: \`${finding.metrics?.line_count ?? 'n/a'}\``);
    lines.push(`  - function count: \`${finding.metrics?.function_count ?? 'n/a'}\``);
    lines.push(`  - peak complexity: \`${finding.metrics?.max_complexity ?? 'n/a'}\``);
    lines.push(`  - fan-out: \`${finding.metrics?.fan_out ?? 'n/a'}\``);
  }
  if (largeFiles.length === 0) {
    lines.push('- none');
  }
  lines.push('');
}

function appendEngineeringDeadPrivate(
  lines,
  { deadPrivateCandidateSets, plausibleDeadPrivate, skepticalDeadPrivate },
) {
  lines.push('## Priority 2: Review Experimental Dead-Private Candidates');
  lines.push('');
  lines.push(
    `- reviewer queue: \`${deadPrivateCandidateSets.sourceLane ?? 'none'}\` (${deadPrivateCandidateSets.selectedCandidates.length} candidate(s), status=${deadPrivateCandidateSets.reviewerLaneStatus ?? 'unknown'})`,
  );
  if (deadPrivateCandidateSets.legacyOnlyCandidates.length > 0) {
    lines.push(
      `- legacy watchlist only: \`${deadPrivateCandidateSets.legacyOnlyCandidates.length}\` additional candidate(s) remain in experimental_findings outside the reviewer queue`,
    );
  }
  if (plausibleDeadPrivate.length > 0) {
    lines.push('- more plausible candidates:');
    appendDeadPrivateCandidateSection(lines, plausibleDeadPrivate, 'symbols');
  }
  if (skepticalDeadPrivate.length > 0) {
    lines.push('- candidates to treat skeptically:');
    appendDeadPrivateCandidateSection(lines, skepticalDeadPrivate, 'symbols');
  }
  if (plausibleDeadPrivate.length === 0 && skepticalDeadPrivate.length === 0) {
    lines.push('- none surfaced in this run');
  }
  lines.push('');
  lines.push(
    'Only convert dead-private suggestions into actual work after a local code read confirms they are truly stale.',
  );
  lines.push('');
}

function appendEngineeringBottomLine(lines) {
  lines.push('## Bottom Line');
  lines.push('');
  lines.push(
    'The highest-value work is not a broad cleanup pass. It is breaking the cycles, fixing the example/template duplication model, and splitting the largest responsibility-heavy files before touching lower-confidence stale-code suggestions.',
  );
  lines.push('');
}
