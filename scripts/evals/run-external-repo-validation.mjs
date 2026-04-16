#!/usr/bin/env node

import { execFile as execFileCallback } from 'node:child_process';
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { promisify } from 'node:util';

import { createMcpSession, runTool } from '../lib/benchmark-harness.mjs';
import { prepareTypeScriptBenchmarkHome } from '../lib/benchmark-plugin-home.mjs';
import { defaultBatchOutputDir } from '../lib/eval-batch.mjs';
import {
  formatSessionTelemetrySummaryMarkdown,
  loadSessionTelemetrySummary,
} from '../lib/session-telemetry.mjs';
import {
  deadPrivateCandidateKey,
  selectDeadPrivateCandidatesFromPayload,
} from './review_dead_private.mjs';

const execFile = promisify(execFileCallback);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const sentruxBin = process.env.SENTRUX_BIN ?? path.join(repoRoot, 'target/debug/sentrux');

export function parseArgs(argv) {
  const result = {
    repoRoot: null,
    repoLabel: null,
    outputDir: null,
    findingsLimit: 25,
    deadPrivateLimit: 10,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--repo-root') {
      index += 1;
      result.repoRoot = argv[index];
      continue;
    }
    if (value === '--repo-label') {
      index += 1;
      result.repoLabel = argv[index];
      continue;
    }
    if (value === '--output-dir') {
      index += 1;
      result.outputDir = argv[index];
      continue;
    }
    if (value === '--findings-limit') {
      index += 1;
      result.findingsLimit = Number(argv[index]);
      continue;
    }
    if (value === '--dead-private-limit') {
      index += 1;
      result.deadPrivateLimit = Number(argv[index]);
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.repoRoot) {
    throw new Error('Missing required --repo-root');
  }

  if (!result.repoLabel) {
    result.repoLabel = path.basename(result.repoRoot);
  }

  return result;
}

function collectFindingKindCounts(findings) {
  const counts = {};

  for (const finding of findings ?? []) {
    const kind = finding?.kind ?? 'unknown';
    counts[kind] = (counts[kind] ?? 0) + 1;
  }

  return counts;
}

function collectDeadPrivateCandidateSets(rawToolAnalysis) {
  const findingsPayload = rawToolAnalysis.findings ?? {};
  const selection = selectDeadPrivateCandidatesFromPayload(findingsPayload);
  const dedupedCandidates = new Map();
  const legacyOnlyCandidates = Array.isArray(selection.legacy_only_candidates)
    ? selection.legacy_only_candidates
    : [];

  for (const candidate of [...selection.candidates, ...legacyOnlyCandidates]) {
    const key = deadPrivateCandidateKey(candidate);
    if (!dedupedCandidates.has(key)) {
      dedupedCandidates.set(key, candidate);
    }
  }

  return {
    sourceLane: selection.source_lane,
    sourceLaneCount: selection.source_lane_count,
    selectedCandidates: selection.candidates,
    consideredLanes: selection.considered_lanes,
    reviewerLaneStatus: selection.reviewer_lane_status,
    reviewerLaneReason: selection.reviewer_lane_reason,
    canonicalCandidateCount: selection.canonical_candidate_count,
    legacyCandidateCount: selection.legacy_candidate_count,
    overlappingCandidateCount: selection.overlapping_candidate_count,
    legacyOnlyCandidates,
    combinedCandidates: [...dedupedCandidates.values()],
  };
}

function sanitizeRepoArtifactLabel(repoLabel) {
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

function sortByNumericField(values, fieldName) {
  return [...values].sort(function compare(left, right) {
    return (right?.[fieldName] ?? 0) - (left?.[fieldName] ?? 0);
  });
}

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

function topDeadPrivateExperimental(rawToolAnalysis, limit) {
  return sortByNumericField(
    collectDeadPrivateCandidateSets(rawToolAnalysis).combinedCandidates,
    'score_0_10000',
  ).slice(0, limit);
}

function deadPrivateSampleSymbols(finding) {
  const sampleEvidence = (finding?.evidence ?? []).find(function findSample(entry) {
    return typeof entry === 'string' && entry.startsWith('sample dead functions: ');
  });

  if (!sampleEvidence) {
    return [];
  }

  return sampleEvidence
    .replace('sample dead functions: ', '')
    .split(',')
    .map(function trimValue(value) {
      return value.trim();
    })
    .filter(Boolean);
}

function hasSuspiciousDeadPrivateSymbols(symbols) {
  if (symbols.length === 0) {
    return false;
  }

  if (symbols.every(function isCell(symbol) {
    return symbol === 'cell';
  })) {
    return true;
  }

  return symbols.some(function isLifecycle(symbol) {
    return symbol === 'getDerivedStateFromError' || symbol === 'componentDidCatch';
  });
}

function deadPrivateFalsePositiveCandidates(rawToolAnalysis) {
  return topDeadPrivateExperimental(rawToolAnalysis, 20).filter(function isSuspicious(finding) {
    const symbols = deadPrivateSampleSymbols(finding);
    return hasSuspiciousDeadPrivateSymbols(symbols);
  });
}

function deadPrivatePlausibleCandidates(rawToolAnalysis) {
  return topDeadPrivateExperimental(rawToolAnalysis, 20).filter(function isPlausible(finding) {
    const symbols = deadPrivateSampleSymbols(finding);
    return symbols.length === 0 || !hasSuspiciousDeadPrivateSymbols(symbols);
  });
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
  const mixedRepoContext = scanSummary.mixed_repo_context ?? scanCoverageBreakdown?.mixed_repo_context ?? {};
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
    for (const finding of deadPrivateFalsePositives.slice(0, 5)) {
      lines.push(
        `  - \`${finding.scope}\` with sample symbols \`${deadPrivateSampleSymbols(finding).join(', ')}\``,
      );
    }
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
    for (const finding of plausibleDeadPrivate) {
      lines.push(
        `  - \`${finding.scope}\` with symbols \`${deadPrivateSampleSymbols(finding).join(', ')}\``,
      );
    }
  }
  if (skepticalDeadPrivate.length > 0) {
    lines.push('- candidates to treat skeptically:');
    for (const finding of skepticalDeadPrivate) {
      lines.push(
        `  - \`${finding.scope}\` with symbols \`${deadPrivateSampleSymbols(finding).join(', ')}\``,
      );
    }
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

async function pathExists(targetPath) {
  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}

async function readJson(targetPath) {
  return JSON.parse(await readFile(targetPath, 'utf8'));
}

async function runNodeScript(scriptPath, args) {
  const { stdout, stderr } = await execFile(process.execPath, [scriptPath, ...args], {
    cwd: repoRoot,
    maxBuffer: 1024 * 1024 * 20,
  });

  return {
    stdout: stdout.trim(),
    stderr: stderr.trim(),
  };
}

async function runGit(repoRootPath, gitArgs) {
  try {
    const { stdout } = await execFile('git', gitArgs, {
      cwd: repoRootPath,
      maxBuffer: 1024 * 1024,
    });

    return stdout.trim();
  } catch {
    return null;
  }
}

async function collectRepoMetadata(repoRootPath) {
  const branch = await runGit(repoRootPath, ['rev-parse', '--abbrev-ref', 'HEAD']);
  const commit = await runGit(repoRootPath, ['rev-parse', '--short', 'HEAD']);
  const status = await runGit(repoRootPath, ['status', '--short']);

  return {
    branch,
    commit,
    workingTreeClean: status === '',
  };
}

async function captureRawToolAnalysis(repoRootPath, findingsLimit) {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'sentrux-external-repo-validation-'));
  const pluginHome = await prepareTypeScriptBenchmarkHome({ tempRoot });
  const session = createMcpSession({
    binPath: sentruxBin,
    repoRoot: repoRootPath,
    homeOverride: pluginHome,
    skipGrammarDownload: process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1',
    requestTimeoutMs: Number(process.env.REQUEST_TIMEOUT_MS ?? '180000'),
  });

  try {
    const analysis = {};
    analysis.scan = (await runTool(session, 'scan', { path: repoRootPath })).payload;
    analysis.check = (await runTool(session, 'check', {})).payload;
    analysis.gate = (await runTool(session, 'gate', {})).payload;
    analysis.findings = (await runTool(session, 'findings', { limit: findingsLimit })).payload;
    await runTool(session, 'session_start', {});
    analysis.session_end = (await runTool(session, 'session_end', {})).payload;

    return analysis;
  } finally {
    await session.close();
    await rm(tempRoot, { recursive: true, force: true });
  }
}

async function maybeBuildSessionTelemetrySummary(repoRootPath, outputDir) {
  const sessionEventsPath = path.join(repoRootPath, '.sentrux', 'agent-session-events.jsonl');
  if (!(await pathExists(sessionEventsPath))) {
    return null;
  }

  const summary = await loadSessionTelemetrySummary(sessionEventsPath, {
    repoRoot: repoRootPath,
  });
  const markdown = formatSessionTelemetrySummaryMarkdown(summary);
  const jsonPath = path.join(outputDir, 'session-telemetry-summary.json');
  const markdownPath = path.join(outputDir, 'session-telemetry-summary.md');
  await writeFile(jsonPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
  await writeFile(markdownPath, markdown, 'utf8');

  return {
    jsonPath,
    markdownPath,
  };
}

async function buildReviewPackets(args, repoRootPath, outputDir) {
  const basePacketArgs = [
    '--repo-root',
    repoRootPath,
    '--limit',
    String(args.findingsLimit),
  ];
  const packetSpecs = [
    {
      tool: 'check',
      jsonPath: path.join(outputDir, 'check-review-packet.json'),
      markdownPath: path.join(outputDir, 'check-review-packet.md'),
    },
    {
      tool: 'findings',
      jsonPath: path.join(outputDir, 'findings-review-packet.json'),
      markdownPath: path.join(outputDir, 'findings-review-packet.md'),
    },
    {
      tool: 'session_end',
      jsonPath: path.join(outputDir, 'session-end-review-packet.json'),
      markdownPath: path.join(outputDir, 'session-end-review-packet.md'),
    },
  ];

  for (const packet of packetSpecs) {
    await runNodeScript(path.join(repoRoot, 'scripts/evals/build-check-review-packet.mjs'), [
      ...basePacketArgs,
      '--tool',
      packet.tool,
      '--output-json',
      packet.jsonPath,
      '--output-md',
      packet.markdownPath,
    ]);
  }

  return {
    checkJsonPath: packetSpecs[0].jsonPath,
    findingsJsonPath: packetSpecs[1].jsonPath,
    sessionEndJsonPath: packetSpecs[2].jsonPath,
  };
}

async function writeExternalValidationArtifacts({
  rawToolAnalysis,
  rawToolSummary,
  scanCoverageBreakdown,
  reportPath,
  validationReport,
  engineeringReport,
  engineeringReportPath,
  repoEngineeringReportPath,
  rawToolAnalysisPath,
  rawToolSummaryPath,
  scanCoverageBreakdownJsonPath,
  scanCoverageBreakdownMarkdownPath,
}) {
  await writeFile(rawToolAnalysisPath, `${JSON.stringify(rawToolAnalysis, null, 2)}\n`, 'utf8');
  await writeFile(rawToolSummaryPath, `${JSON.stringify(rawToolSummary, null, 2)}\n`, 'utf8');
  await writeFile(
    scanCoverageBreakdownJsonPath,
    `${JSON.stringify(scanCoverageBreakdown, null, 2)}\n`,
    'utf8',
  );
  await writeFile(
    scanCoverageBreakdownMarkdownPath,
    formatScanCoverageBreakdownMarkdown(scanCoverageBreakdown),
    'utf8',
  );
  await writeFile(reportPath, validationReport, 'utf8');
  await writeFile(engineeringReportPath, engineeringReport, 'utf8');
  await writeFile(repoEngineeringReportPath, engineeringReport, 'utf8');
}

async function main() {
  const args = parseArgs(process.argv);
  const repoRootPath = path.resolve(args.repoRoot);
  const outputDir = path.resolve(
    args.outputDir ??
      defaultBatchOutputDir(repoRootPath, 'external-repo-validation', args.repoLabel),
  );
  const metadata = await collectRepoMetadata(repoRootPath);

  await mkdir(outputDir, { recursive: true });

  const deadPrivatePath = path.join(outputDir, 'dead-private-dry-run.json');
  const rawToolAnalysisPath = path.join(outputDir, 'raw-tool-analysis.json');
  const rawToolSummaryPath = path.join(outputDir, 'raw-tool-summary.json');
  const scanCoverageBreakdownJsonPath = path.join(outputDir, 'scan-coverage-breakdown.json');
  const scanCoverageBreakdownMarkdownPath = path.join(outputDir, 'scan-coverage-breakdown.md');
  const reportPath = path.join(outputDir, 'REPORT.md');
  const engineeringReportPath = path.join(outputDir, 'ENGINEERING_REPORT.md');
  const repoEngineeringReportPath = path.join(
    outputDir,
    `${sanitizeRepoArtifactLabel(args.repoLabel)}_ENGINEERING_REPORT.md`,
  );

  const reviewPackets = await buildReviewPackets(args, repoRootPath, outputDir);
  await runNodeScript(path.join(repoRoot, 'scripts/evals/review_dead_private.mjs'), [
    '--repo-root',
    repoRootPath,
    '--repo-name',
    args.repoLabel,
    '--limit',
    String(args.deadPrivateLimit),
    '--findings-limit',
    String(Math.max(args.findingsLimit, args.deadPrivateLimit)),
    '--dry-run',
    '--output',
    deadPrivatePath,
  ]);

  const findingsReviewPacket = await readJson(reviewPackets.findingsJsonPath);
  const packetValidation = buildPacketValidation(findingsReviewPacket);
  const rawToolAnalysis = await captureRawToolAnalysis(repoRootPath, Math.max(args.findingsLimit, 50));
  const rawToolSummary = buildRawToolSummary(rawToolAnalysis);
  const scanCoverageBreakdown = buildScanCoverageBreakdown(rawToolAnalysis);
  const engineeringReport = buildEngineeringReport({
    repoRootPath,
    repoLabel: args.repoLabel,
    branch: metadata.branch,
    commit: metadata.commit,
    rawToolAnalysis,
  });
  const validationReport = buildValidationReport({
    repoRootPath,
    repoLabel: args.repoLabel,
    branch: metadata.branch,
    commit: metadata.commit,
    workingTreeClean: metadata.workingTreeClean,
    rawToolAnalysis,
    rawToolSummary,
    packetValidation,
    scanCoverageBreakdown,
  });

  await maybeBuildSessionTelemetrySummary(repoRootPath, outputDir);
  await writeExternalValidationArtifacts({
    rawToolAnalysis,
    rawToolSummary,
    scanCoverageBreakdown,
    reportPath,
    validationReport,
    engineeringReport,
    engineeringReportPath,
    repoEngineeringReportPath,
    rawToolAnalysisPath,
    rawToolSummaryPath,
    scanCoverageBreakdownJsonPath,
    scanCoverageBreakdownMarkdownPath,
  });

  console.log(JSON.stringify({
    output_dir: outputDir,
    report_path: reportPath,
    engineering_report_path: engineeringReportPath,
    repo_engineering_report_path: repoEngineeringReportPath,
    scan_coverage_breakdown_json_path: scanCoverageBreakdownJsonPath,
    scan_coverage_breakdown_markdown_path: scanCoverageBreakdownMarkdownPath,
  }, null, 2));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(function handleError(error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
