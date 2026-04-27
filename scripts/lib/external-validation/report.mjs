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

function hasText(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function asArray(value) {
  if (Array.isArray(value)) {
    return value;
  }
  if (hasText(value)) {
    return [value];
  }
  return [];
}

function formatOptionalList(value, delimiter = ', ') {
  const items = asArray(value);
  return items.length > 0 ? items.join(delimiter) : 'not specified';
}

function selectPatchBrief(rawToolAnalysis) {
  return (
    rawToolAnalysis.briefs?.pre_merge ??
    rawToolAnalysis.briefs?.patch ??
    rawToolAnalysis.brief_pre_merge ??
    rawToolAnalysis.brief_patch ??
    null
  );
}

function selectOnboardingBrief(rawToolAnalysis) {
  return rawToolAnalysis.briefs?.repo_onboarding ?? rawToolAnalysis.brief_repo_onboarding ?? null;
}

function primaryBriefTargets(rawToolAnalysis) {
  const brief = selectPatchBrief(rawToolAnalysis);
  return Array.isArray(brief?.primary_targets) ? brief.primary_targets : [];
}

function onboardingWatchpoints(rawToolAnalysis) {
  const brief = selectOnboardingBrief(rawToolAnalysis);
  return Array.isArray(brief?.watchpoints) ? brief.watchpoints : [];
}

function patchWatchpoints(rawToolAnalysis) {
  const brief = selectPatchBrief(rawToolAnalysis);
  return Array.isArray(brief?.watchpoints) ? brief.watchpoints : [];
}

function doNotChaseItems(rawToolAnalysis) {
  const brief = selectPatchBrief(rawToolAnalysis) ?? selectOnboardingBrief(rawToolAnalysis);
  return Array.isArray(brief?.do_not_chase) ? brief.do_not_chase : [];
}

function findingIdentity(finding) {
  if (typeof finding === 'string') {
    return finding;
  }
  const file = asArray(finding.files)[0];
  return [
    finding.kind ?? 'unknown',
    finding.scope ?? finding.concept_id ?? file ?? 'unknown',
    finding.summary ?? finding.message ?? '',
  ].join(':');
}

function dedupeFindings(findings) {
  const seen = new Set();
  const deduped = [];
  for (const finding of findings) {
    const identity = findingIdentity(finding);
    if (seen.has(identity)) {
      continue;
    }
    seen.add(identity);
    deduped.push(finding);
  }
  return deduped;
}

function missingObligations(rawToolAnalysis) {
  const gateObligations = rawToolAnalysis.gate?.missing_obligations;
  if (Array.isArray(gateObligations)) {
    return gateObligations;
  }

  const brief = selectPatchBrief(rawToolAnalysis);
  return Array.isArray(brief?.missing_obligations) ? brief.missing_obligations : [];
}

function formatFixSites(target) {
  return formatOptionalList(asArray(target.likely_fix_sites).slice(0, 5));
}

function formatRepairCompleteness(target) {
  const completeness = target.repair_packet?.completeness_0_10000;
  if (!Number.isFinite(completeness)) {
    return 'not specified';
  }

  return `${completeness} / 10000`;
}

function targetLane(target) {
  return (
    target.primary_lane ??
    target.presentation_class ??
    target.default_surface_role ??
    target.trust_tier ??
    'not specified'
  );
}

function likelyCycleCut(finding) {
  const candidate = finding.cut_candidates?.[0];
  if (!candidate) {
    return {
      summary: 'inspect the candidate back-edge and split contracts from implementations',
      source: null,
      target: null,
      reduction: null,
      remaining: null,
    };
  }

  return {
    summary: candidate.summary,
    source: candidate.source,
    target: candidate.target,
    reduction: candidate.reduction_file_count,
    remaining: candidate.remaining_cycle_size,
  };
}

function hotspotSignals(finding) {
  const metrics = finding.metrics ?? {};
  return [
    metrics.authority_breadth ? `authority surfaces=${metrics.authority_breadth}` : null,
    metrics.side_effect_breadth ? `side-effect targets=${metrics.side_effect_breadth}` : null,
    metrics.timer_retry_weight ? `timer/retry signals=${metrics.timer_retry_weight}` : null,
    metrics.async_branch_weight ? `async/branching signals=${metrics.async_branch_weight}` : null,
    metrics.max_complexity ? `max complexity=${metrics.max_complexity}` : null,
    metrics.churn_commits ? `recent churn commits=${metrics.churn_commits}` : null,
    metrics.hotspot_risk ? `git hotspot risk=${metrics.hotspot_risk}` : null,
  ].filter(Boolean);
}

function largeFileRole(finding) {
  const scope = String(finding.scope ?? '');
  const extension = scope.includes('.') ? scope.split('.').pop().toLowerCase() : '';
  const codeLike = ['js', 'jsx', 'ts', 'tsx', 'mjs', 'cjs', 'py', 'rs', 'go', 'java', 'kt', 'swift'];
  if (codeLike.includes(extension)) {
    return 'code responsibility surface';
  }

  if (
    (finding.metrics?.function_count ?? 0) === 0 &&
    (finding.metrics?.max_complexity ?? 0) === 0
  ) {
    return 'data/config asset';
  }

  return 'mixed maintenance surface';
}

function largeFileFirstCut(role) {
  if (role === 'data/config asset') {
    return 'do not split solely for line count; split only if fixture ownership, generation, or review friction is real';
  }

  return 'extract one cohesive responsibility with tests before attempting broad file decomposition';
}

function obligationConcept(obligation) {
  return obligation.concept_id ?? obligation.concept ?? obligation.scope ?? 'unknown';
}

function obligationSites(obligation) {
  const sites = asArray(obligation.missing_sites ?? obligation.required_update_sites);
  return sites
    .slice(0, 5)
    .map((site) => (typeof site === 'string' ? site : site.path ?? JSON.stringify(site)));
}

function appendCodeBullet(lines, label, value) {
  lines.push(`- ${label}: \`${value}\``);
}

function appendNestedDetail(lines, label, value) {
  lines.push(`  - ${label}: ${value}`);
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
  rawToolSummary = null,
  scanCoverageBreakdown = null,
  setupPreflight = null,
  advisorEvidence = null,
}) {
  const largeFiles = topLargeFiles(rawToolAnalysis, 3);
  const cycles = topCycles(rawToolAnalysis, 2);
  const clones = topClones(rawToolAnalysis, 10);
  const primaryTargets = primaryBriefTargets(rawToolAnalysis);
  const obligations = missingObligations(rawToolAnalysis);
  const watchpoints = dedupeFindings([
    ...patchWatchpoints(rawToolAnalysis),
    ...onboardingWatchpoints(rawToolAnalysis),
  ]);
  const doNotChase = dedupeFindings(doNotChaseItems(rawToolAnalysis));
  const deadPrivateCandidateSets = collectDeadPrivateCandidateSets(rawToolAnalysis);
  const plausibleDeadPrivate = deadPrivatePlausibleCandidates(rawToolAnalysis).slice(0, 5);
  const skepticalDeadPrivate = deadPrivateFalsePositiveCandidates(rawToolAnalysis).slice(0, 5);
  const lines = [];

  appendEngineeringScope(lines, { repoRootPath, repoLabel, branch, commit });
  appendEngineeringSetup(lines, setupPreflight);
  appendEngineeringLaneMap(lines, {
    primaryTargets,
    obligations,
    watchpoints,
    doNotChase,
    advisorEvidence,
  });
  appendEngineeringImmediateActions(lines, primaryTargets, obligations);
  appendEngineeringCycles(lines, cycles);
  appendEngineeringCloneDrift(lines, clones);
  appendEngineeringHotspots(lines, watchpoints);
  appendEngineeringLargeFiles(lines, largeFiles);
  appendEngineeringDeadPrivate(lines, {
    deadPrivateCandidateSets,
    plausibleDeadPrivate,
    skepticalDeadPrivate,
  });
  appendEngineeringConfidence(lines, {
    rawToolAnalysis,
    rawToolSummary,
    scanCoverageBreakdown,
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
    'Use the immediate patch actions first when present; they are the narrowest repair queue surfaced by the analysis.',
    'Structural work remains useful, but should not crowd out concrete propagation, boundary, clone, or obligation follow-through.',
    'Lower-confidence work: audit dead-private candidates manually instead of applying automated cleanup blindly.',
  ]);
}

function appendEngineeringSetup(lines, setupPreflight) {
  if (!setupPreflight) {
    return;
  }

  lines.push('## Setup And Analysis Safety');
  lines.push('');
  appendCodeBullet(lines, 'setup preflight', setupPreflight.passed ? 'pass' : 'fail');
  appendCodeBullet(lines, 'analysis mode', setupPreflight.analysis_mode ?? 'unknown');
  appendCodeBullet(
    lines,
    'target repo mutation',
    setupPreflight.safety?.mutates_target_repo ? 'possible' : 'none by default',
  );
  appendCodeBullet(
    lines,
    'isolated workspace',
    setupPreflight.safety?.isolated_workspace ? 'yes' : 'no',
  );
  const warningChecks = (setupPreflight.checks ?? []).filter((check) => check.status === 'warn');
  if (warningChecks.length > 0) {
    lines.push('- setup warnings:');
    for (const check of warningChecks) {
      appendNestedDetail(lines, check.id, check.detail);
    }
  }
  lines.push('');
}

function appendEngineeringLaneMap(
  lines,
  { primaryTargets, obligations, watchpoints, doNotChase, advisorEvidence },
) {
  lines.push('## Actionability Map');
  lines.push('');
  lines.push(`- required actions: \`${primaryTargets.length + obligations.length}\``);
  lines.push(`- recommended maintainability work: \`${watchpoints.length}\``);
  lines.push(`- do-not-chase items: \`${doNotChase.length}\``);
  if (advisorEvidence?.default_lane) {
    const actionCount = advisorEvidence.default_lane.primary_action_count;
    const actionLimit = advisorEvidence.default_lane.max_primary_actions;
    lines.push(
      `- default lane: \`${actionCount}/${actionLimit}\` primary actions under current policy`,
    );
    lines.push(
      `- large-file primary slots: \`${advisorEvidence.default_lane.large_file_primary_slot_count}\``,
    );
  }
  lines.push('');
  if (primaryTargets.length === 0 && obligations.length === 0) {
    lines.push(
      'No required patch actions surfaced. Treat the remaining items as maintainability backlog unless they overlap with current work.',
    );
    lines.push('');
    return;
  }
  lines.push(
    'Work top-down: required patch actions first, then concrete maintainability work, then manual-review watchpoints.',
  );
  lines.push('');
}

function appendEngineeringImmediateActions(lines, primaryTargets, obligations) {
  lines.push('## Priority 1: Complete The Current Patch Follow-Through');
  lines.push('');
  if (primaryTargets.length === 0 && obligations.length === 0) {
    lines.push('- no immediate patch-specific blockers surfaced');
    lines.push('');
    return;
  }

  for (const target of primaryTargets) {
    lines.push(`### \`${target.kind ?? 'unknown'}\` in \`${target.scope ?? 'unknown'}\``);
    lines.push('');
    lines.push(`- summary: ${target.summary ?? 'no summary'}`);
    appendNestedDetail(lines, 'lane', `\`${targetLane(target)}\``);
    appendNestedDetail(lines, 'trust', `\`${target.trust_tier ?? target.confidence ?? 'not specified'}\``);
    appendNestedDetail(lines, 'severity', `\`${target.severity ?? 'not specified'}\``);
    appendNestedDetail(lines, 'repair completeness', `\`${formatRepairCompleteness(target)}\``);
    lines.push(`- why it matters now: ${formatOptionalList(target.why_now)}`);
    lines.push(`- likely fix sites: ${formatFixSites(target)}`);
    if (target.repair_packet?.smallest_safe_first_cut) {
      appendNestedDetail(lines, 'smallest safe first cut', target.repair_packet.smallest_safe_first_cut);
    }
    if (Array.isArray(target.repair_packet?.verify_after) && target.repair_packet.verify_after.length > 0) {
      appendNestedDetail(lines, 'verify after', target.repair_packet.verify_after.slice(0, 3).join('; '));
    }
    lines.push('');
  }

  if (obligations.length > 0) {
    lines.push('### Concrete Follow-Through Surfaces');
    lines.push('');
    for (const obligation of obligations.slice(0, 10)) {
      lines.push(
        `- \`${obligationConcept(obligation)}\`: ${obligation.summary ?? obligation.message ?? 'missing follow-through'}`,
      );
      const sites = obligationSites(obligation);
      if (sites.length > 0) {
        appendNestedDetail(lines, 'update sites', sites.join(', '));
      }
    }
    if (obligations.length > 10) {
      lines.push(`- ${obligations.length - 10} additional obligation(s) omitted from this summary`);
    }
    lines.push('');
  }
}

function appendEngineeringCycles(lines, cycles) {
  lines.push('## Priority 2: Break The Dependency Cycles');
  lines.push('');
  for (const finding of cycles) {
    const cut = likelyCycleCut(finding);
    lines.push(`### \`${finding.scope}\``);
    lines.push('');
    lines.push(`- summary: ${finding.summary}`);
    lines.push(`- impact: ${finding.impact}`);
    lines.push(`- best first cut: ${cut.summary}`);
    if (cut.source && cut.target) {
      appendNestedDetail(lines, 'seam', `\`${cut.source} -> ${cut.target}\``);
    }
    if (Number.isFinite(cut.reduction)) {
      appendNestedDetail(lines, 'expected reduction', `removes ${cut.reduction} cyclic file(s)`);
    }
    if (Number.isFinite(cut.remaining)) {
      appendNestedDetail(lines, 'remaining cycle size after cut', `\`${cut.remaining}\``);
    }
    appendNestedDetail(
      lines,
      'do not over-refactor',
      'cut one seam, rerun analysis, and avoid untangling the whole cluster in one pass',
    );
    lines.push('');
  }
  if (cycles.length === 0) {
    lines.push('- none');
    lines.push('');
  }
}

function appendEngineeringCloneDrift(lines, clones) {
  lines.push('## Priority 3: Reduce Duplicate Logic Drift');
  lines.push('');
  for (const finding of clones.slice(0, 5)) {
    const cloneScope = asArray(finding.files).join(' | ') || finding.scope || 'unknown';
    const driftReasons = formatOptionalList(finding.reasons, '; ');
    lines.push(`- \`${cloneScope}\``);
    appendNestedDetail(lines, 'total cloned lines', `\`${finding.total_lines ?? 'n/a'}\``);
    if (Array.isArray(finding.instances) && finding.instances.length > 0) {
      appendNestedDetail(
        lines,
        'instances',
        finding.instances
          .slice(0, 5)
          .map(function formatCloneInstance(instance) {
            const file = instance.file ?? 'unknown';
            const func = instance.func ?? 'block';
            const lines = instance.lines ?? 'n/a';
            return `${file}:${func}:${lines} lines`;
          })
          .join(', '),
      );
    }
    appendNestedDetail(lines, 'drift reasons', `\`${driftReasons}\``);
    appendNestedDetail(
      lines,
      'smallest safe first cut',
      'decide whether the copies must stay behaviorally identical before extracting a shared helper',
    );
  }
  if (clones.length === 0) {
    lines.push('- none');
  }
  lines.push('');
  lines.push('- recommendation: pick one canonical source for shared example logic and enforce sync mechanically');
  lines.push('');
}

function appendEngineeringHotspots(lines, watchpoints) {
  const hotspots = watchpoints.filter((finding) => finding.kind === 'hotspot');
  lines.push('## Coordination Hotspot Watchpoints');
  lines.push('');
  if (hotspots.length === 0) {
    lines.push('- none');
    lines.push('');
    return;
  }
  for (const finding of hotspots.slice(0, 5)) {
    lines.push(`- \`${finding.scope ?? asArray(finding.files)[0] ?? 'unknown'}\``);
    lines.push(`  - summary: ${finding.summary ?? finding.message ?? 'coordination hotspot'}`);
    if (finding.impact) {
      appendNestedDetail(lines, 'impact', finding.impact);
    }
    const signals = hotspotSignals(finding);
    if (signals.length > 0) {
      appendNestedDetail(lines, 'top pressure contributors', signals.join(', '));
    }
    appendNestedDetail(
      lines,
      'smallest safe first cut',
      'extract one clear seam such as health/status, adapter normalization, retry policy, or request orchestration',
    );
  }
  lines.push('');
}

function appendEngineeringLargeFiles(lines, largeFiles) {
  lines.push('## Priority 4: Review The Largest Files By Maintenance Role');
  lines.push('');
  for (const finding of largeFiles) {
    const role = largeFileRole(finding);
    lines.push(`- \`${finding.scope}\``);
    appendNestedDetail(lines, 'role', `\`${role}\``);
    appendNestedDetail(lines, 'line count', `\`${finding.metrics?.line_count ?? 'n/a'}\``);
    appendNestedDetail(lines, 'function count', `\`${finding.metrics?.function_count ?? 'n/a'}\``);
    appendNestedDetail(lines, 'peak complexity', `\`${finding.metrics?.max_complexity ?? 'n/a'}\``);
    appendNestedDetail(lines, 'fan-out', `\`${finding.metrics?.fan_out ?? 'n/a'}\``);
    appendNestedDetail(lines, 'smallest safe first cut', largeFileFirstCut(role));
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
  lines.push('## Priority 5: Review Experimental Dead-Private Candidates');
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

function appendEngineeringConfidence(
  lines,
  { rawToolAnalysis, rawToolSummary, scanCoverageBreakdown },
) {
  const confidence =
    rawToolAnalysis.briefs?.pre_merge?.confidence ??
    rawToolAnalysis.gate?.confidence ??
    rawToolAnalysis.scan?.confidence ??
    {};
  const scanTrust =
    rawToolAnalysis.scan?.scan_trust ??
    scanCoverageBreakdown?.candidate_file_coverage ??
    {};
  lines.push('## Tool Confidence And Coverage');
  lines.push('');
  const scanConfidence =
    confidence.scan_confidence_0_10000 ?? scanTrust.overall_confidence_0_10000 ?? 'n/a';
  appendCodeBullet(lines, 'scan confidence', `${scanConfidence} / 10000`);
  appendCodeBullet(lines, 'rule coverage', `${confidence.rule_coverage_0_10000 ?? 'n/a'} / 10000`);
  appendCodeBullet(lines, 'semantic rules loaded', confidence.semantic_rules_loaded ?? 'n/a');
  appendCodeBullet(
    lines,
    'kept files',
    `${scanTrust.kept_files ?? 'n/a'} / ${scanTrust.candidate_files ?? 'n/a'}`,
  );
  if (rawToolSummary?.findings_summary?.kind_counts) {
    appendNestedDetail(
      lines,
      'finding kinds',
      Object.entries(rawToolSummary.findings_summary.kind_counts)
        .map(([kind, count]) => `${kind}=${count}`)
        .join(', '),
    );
  }
  lines.push('');
}

function appendEngineeringBottomLine(lines) {
  lines.push('## Bottom Line');
  lines.push('');
  lines.push(
    'The highest-value work is the smallest concrete follow-through queue first, then structural backlog work. Do not turn broad cleanup or lower-confidence stale-code suggestions into the lead task unless the patch-specific actions are already resolved.',
  );
  lines.push('');
}
