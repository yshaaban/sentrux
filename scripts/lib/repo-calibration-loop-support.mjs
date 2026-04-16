import { copyFile, mkdir, rename, rm, writeFile } from 'node:fs/promises';
import { execFile as execFileCallback } from 'node:child_process';
import path from 'node:path';
import { promisify } from 'node:util';
import { resolveManifestPath } from './eval-batch.mjs';
import { pathExists, readJson } from './repo-calibration-artifacts.mjs';

const execFile = promisify(execFileCallback);

export function resolveRepoArtifactPath(repoRootPath, relativePath) {
  if (!relativePath) {
    return null;
  }

  return path.resolve(repoRootPath, relativePath);
}

function isVerdictTemplatePath(targetPath) {
  return Boolean(targetPath) && path.basename(targetPath).includes('.template.');
}

function deriveCompanionPath(targetPath, extension) {
  if (!targetPath) {
    return null;
  }

  const parsed = path.parse(targetPath);
  if (parsed.ext === extension) {
    return targetPath;
  }

  return path.join(parsed.dir, `${parsed.name}${extension}`);
}

function buildRunArtifactPath(outputDir, baseName, extension) {
  return path.join(outputDir, `${baseName}${extension}`);
}

function resolveManifestArtifactPaths(manifestPath, manifest, artifactConfig) {
  return {
    cohortManifestPath: resolveManifestPath(manifestPath, manifest.cohort_manifest),
    codexBatchManifestPath: resolveManifestPath(
      manifestPath,
      manifest.live_batch_manifest ?? manifest.codex_batch_manifest,
    ),
    replayBatchManifestPath: resolveManifestPath(
      manifestPath,
      manifest.replay_batch_manifest,
    ),
    reviewVerdictsPath: resolveManifestPath(
      manifestPath,
      artifactConfig.review_verdicts_input ?? manifest.review_verdicts,
    ),
    defectReportPath: resolveManifestPath(
      manifestPath,
      artifactConfig.seeded_defect_report ?? manifest.defect_report,
    ),
    remediationReportPath: resolveManifestPath(
      manifestPath,
      artifactConfig.remediation_report ?? manifest.remediation_report,
    ),
    benchmarkPath: resolveManifestPath(
      manifestPath,
      artifactConfig.benchmark_artifact ?? manifest.benchmark_artifact,
    ),
  };
}

function resolveStableArtifactPaths(repoRootPath, artifactConfig) {
  const configuredReviewPacketPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.review_packet_output,
  );
  const configuredScorecardPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.scorecard_output,
  );
  const configuredBacklogPath = resolveRepoArtifactPath(
    repoRootPath,
    artifactConfig.backlog_output,
  );

  return {
    stableReviewVerdictsOutputPath: resolveRepoArtifactPath(
      repoRootPath,
      artifactConfig.review_verdicts_output,
    ),
    stableReviewPacketJsonPath: deriveCompanionPath(
      configuredReviewPacketPath,
      '.json',
    ),
    stableReviewPacketMarkdownPath: deriveCompanionPath(
      configuredReviewPacketPath,
      '.md',
    ),
    stableScorecardJsonPath: deriveCompanionPath(configuredScorecardPath, '.json'),
    stableScorecardMarkdownPath: deriveCompanionPath(
      configuredScorecardPath,
      '.md',
    ),
    stableBacklogJsonPath: deriveCompanionPath(configuredBacklogPath, '.json'),
    stableBacklogMarkdownPath: deriveCompanionPath(configuredBacklogPath, '.md'),
  };
}

function resolveRunArtifactPaths(outputDir) {
  return {
    reviewPacketJsonPath: buildRunArtifactPath(
      outputDir,
      'check-review-packet',
      '.json',
    ),
    reviewPacketMarkdownPath: buildRunArtifactPath(
      outputDir,
      'check-review-packet',
      '.md',
    ),
    runReviewVerdictsOutputPath: buildRunArtifactPath(
      outputDir,
      'review-verdicts',
      '.json',
    ),
    mergedTelemetryJsonPath: path.join(outputDir, 'session-telemetry-summary.json'),
    mergedTelemetryMarkdownPath: path.join(outputDir, 'session-telemetry-summary.md'),
    scorecardJsonPath: buildRunArtifactPath(outputDir, 'signal-scorecard', '.json'),
    scorecardMarkdownPath: buildRunArtifactPath(outputDir, 'signal-scorecard', '.md'),
    backlogJsonPath: buildRunArtifactPath(outputDir, 'signal-backlog', '.json'),
    backlogMarkdownPath: buildRunArtifactPath(outputDir, 'signal-backlog', '.md'),
  };
}

function resolveBatchArtifactPaths(outputDir) {
  const codexBatchOutputDir = path.join(outputDir, 'codex-batch');
  const replayBatchOutputDir = path.join(outputDir, 'replay-batch');

  return {
    codexBatchOutputDir,
    replayBatchOutputDir,
    codexBatchResultPath: path.join(
      codexBatchOutputDir,
      'codex-session-batch.json',
    ),
    replayBatchResultPath: path.join(
      replayBatchOutputDir,
      'diff-replay-batch.json',
    ),
  };
}

export function resolveLoopArtifactPaths({
  manifest,
  manifestPath,
  repoRootPath,
  outputDir,
}) {
  const artifactConfig = manifest.artifacts ?? {};
  return {
    artifactConfig,
    ...resolveManifestArtifactPaths(manifestPath, manifest, artifactConfig),
    ...resolveStableArtifactPaths(repoRootPath, artifactConfig),
    ...resolveRunArtifactPaths(outputDir),
    ...resolveBatchArtifactPaths(outputDir),
  };
}

export async function acquireLoopLock(lockPath, metadata) {
  try {
    await mkdir(lockPath);
  } catch (error) {
    if (error && typeof error === 'object' && error.code === 'EEXIST') {
      throw new Error(`Another calibration loop already holds the repo lock: ${lockPath}`);
    }
    throw error;
  }

  await writeFile(
    path.join(lockPath, 'owner.json'),
    `${JSON.stringify(metadata, null, 2)}\n`,
    'utf8',
  );
  return async function releaseLoopLock() {
    await rm(lockPath, { recursive: true, force: true });
  };
}

async function publishArtifact(sourcePath, targetPath) {
  if (!sourcePath || !targetPath) {
    return;
  }

  await mkdir(path.dirname(targetPath), { recursive: true });
  const tempPath = `${targetPath}.tmp-${process.pid}-${Date.now()}`;
  await copyFile(sourcePath, tempPath);
  await rename(tempPath, targetPath);
}

export async function publishArtifacts(pairs) {
  for (const pair of pairs) {
    if (!pair?.sourcePath || !pair?.targetPath) {
      continue;
    }

    await publishArtifact(pair.sourcePath, pair.targetPath);
  }
}

export function buildBatchRunArgs(
  manifestPath,
  outputDir,
  cohortManifestPath,
  cohortId,
) {
  const args = ['--manifest', manifestPath, '--output-dir', outputDir];

  if (cohortManifestPath) {
    args.push('--cohort-manifest', cohortManifestPath);
  }
  if (cohortId) {
    args.push('--cohort-id', cohortId);
  }

  return args;
}

function buildNodeArgs(scriptPath, args) {
  return [scriptPath, ...args];
}

export async function runNodeScript(repoRoot, scriptPath, args) {
  const { stdout, stderr } = await execFile(process.execPath, buildNodeArgs(scriptPath, args), {
    cwd: repoRoot,
    maxBuffer: 1024 * 1024 * 20,
  });

  return {
    script: path.relative(repoRoot, scriptPath),
    args,
    stdout: stdout.trim(),
    stderr: stderr.trim(),
  };
}

export function buildSummaryMarkdown(summary) {
  const lines = [];
  lines.push('# Repo Calibration Loop');
  lines.push('');
  lines.push(`- repo id: \`${summary.repo_id}\``);
  lines.push(`- repo root: \`${summary.repo_root}\``);
  lines.push(`- generated at: \`${summary.generated_at}\``);
  lines.push(`- output dir: \`${summary.output_dir}\``);
  lines.push('');
  lines.push('## Artifacts');
  lines.push('');
  lines.push(`- live batch: ${summary.artifacts.codex_batch_json ? `\`${summary.artifacts.codex_batch_json}\`` : 'skipped'}`);
  lines.push(`- replay batch: ${summary.artifacts.replay_batch_json ? `\`${summary.artifacts.replay_batch_json}\`` : 'skipped'}`);
  lines.push(`- merged telemetry: ${summary.artifacts.session_telemetry_json ? `\`${summary.artifacts.session_telemetry_json}\`` : 'none'}`);
  lines.push(`- review packet: ${summary.artifacts.review_packet_json ? `\`${summary.artifacts.review_packet_json}\`` : 'skipped'}`);
  lines.push(`- scorecard: ${summary.artifacts.scorecard_json ? `\`${summary.artifacts.scorecard_json}\`` : 'skipped'}`);
  lines.push(`- backlog: ${summary.artifacts.backlog_json ? `\`${summary.artifacts.backlog_json}\`` : 'skipped'}`);
  lines.push('');
  lines.push('## Summary');
  lines.push('');
  lines.push(`- total sessions: ${summary.summary.session_count}`);
  lines.push(`- total signals: ${summary.summary.total_signals ?? 0}`);
  lines.push(`- weak signals: ${summary.summary.weak_signal_count ?? 0}`);
  lines.push(`- review samples: ${summary.summary.review_sample_count ?? 0}`);
  lines.push(`- live clean rate: ${summary.summary.live_clean_rate ?? 'n/a'}`);
  lines.push(`- replay clean rate: ${summary.summary.replay_clean_rate ?? 'n/a'}`);
  lines.push(`- next signal: ${summary.summary.recommended_next_signal ?? 'none'}`);
  lines.push('');

  if (summary.delta) {
    lines.push('## Delta');
    lines.push('');
    lines.push(`- total signals delta: ${summary.delta.total_signals?.delta ?? 'n/a'}`);
    lines.push(`- weak signals delta: ${summary.delta.weak_signal_count?.delta ?? 'n/a'}`);
    lines.push(`- review samples delta: ${summary.delta.review_sample_count?.delta ?? 'n/a'}`);
    lines.push(`- live clean rate delta: ${summary.delta.live_clean_rate?.delta ?? 'n/a'}`);
    lines.push(`- replay clean rate delta: ${summary.delta.replay_clean_rate?.delta ?? 'n/a'}`);
    lines.push(`- next signal changed: ${summary.delta.recommended_next_signal?.changed ? 'yes' : 'no'}`);
    if (Array.isArray(summary.delta.recommendation_changes) && summary.delta.recommendation_changes.length > 0) {
      lines.push(`- recommendation changes: ${summary.delta.recommendation_changes.map((entry) => `${entry.signal_kind}:${entry.previous}->${entry.current}`).join(', ')}`);
    }
    lines.push('');
  }

  if (Array.isArray(summary.warnings) && summary.warnings.length > 0) {
    lines.push('## Warnings');
    lines.push('');
    for (const warning of summary.warnings) {
      lines.push(`- ${warning}`);
    }
    lines.push('');
  }

  return `${lines.join('\n')}\n`;
}

function resolveBatchManifestKeys(idKey) {
  if (idKey === 'task_id') {
    return {
      manifestEntriesKey: 'tasks',
      manifestIdKey: 'task_id',
    };
  }

  return {
    manifestEntriesKey: 'replays',
    manifestIdKey: 'replay_id',
  };
}

function normalizeExpectedSignalKinds(expectedSignalKinds) {
  return [...new Set((expectedSignalKinds ?? []).filter(Boolean))].sort();
}

export function buildBatchExpectationWarnings(
  batchManifest,
  batchResult,
  idKey,
  laneLabel,
) {
  if (!batchManifest || !batchResult) {
    return [];
  }

  const { manifestEntriesKey, manifestIdKey } = resolveBatchManifestKeys(idKey);
  const manifestEntries = Array.isArray(batchManifest[manifestEntriesKey])
    ? batchManifest[manifestEntriesKey]
    : [];
  const manifestById = new Map(
    manifestEntries.map((entry) => [entry[manifestIdKey], entry]),
  );
  const warnings = [];

  for (const result of batchResult.results ?? []) {
    const resultId = result[idKey];
    if (!resultId) {
      warnings.push(`${laneLabel} batch result missing ${idKey}`);
      continue;
    }

    const manifestEntry = manifestById.get(resultId);
    if (!manifestEntry) {
      warnings.push(`${laneLabel} batch result ${resultId} is not present in the manifest`);
      continue;
    }

    const expectedFromManifest = normalizeExpectedSignalKinds(
      manifestEntry.expected_signal_kinds ?? batchManifest.expected_signal_kinds,
    );
    const expectedFromResult = normalizeExpectedSignalKinds(result.expected_signal_kinds);

    if (expectedFromManifest.join('|') !== expectedFromResult.join('|')) {
      warnings.push(
        `${laneLabel} batch result ${resultId} expected_signal_kinds drifted: manifest=[${expectedFromManifest.join(', ')}] result=[${expectedFromResult.join(', ')}]`,
      );
    }
  }

  return warnings;
}

export function buildBatchFailureWarnings(batchResult, laneLabel) {
  return (batchResult?.failures ?? []).map((failure) => {
    const failureId = failure.task_id ?? failure.replay_id ?? failure.task_label ?? 'unknown';
    return `${laneLabel} lane failure for ${failureId}: ${failure.error_message ?? failure.status ?? 'unknown error'}`;
  });
}

export async function selectReviewVerdictsPath(outputPath, inputPath) {
  if (inputPath && !isVerdictTemplatePath(inputPath) && (await pathExists(inputPath))) {
    return inputPath;
  }
  if (outputPath && (await pathExists(outputPath))) {
    return outputPath;
  }

  return null;
}

function countReviewSamples(reviewPacket) {
  return reviewPacket?.summary?.sample_count ?? reviewPacket?.samples?.length ?? 0;
}

export async function maybeBuildProvisionalReviewVerdicts({
  repoRoot,
  manifest,
  reviewPacketJsonPath,
  reviewPacketMarkdownPath,
  reviewVerdictsOutputPath,
  reviewVerdictsPath,
}) {
  const hasCuratedReviewVerdicts =
    reviewVerdictsPath &&
    !isVerdictTemplatePath(reviewVerdictsPath) &&
    (await pathExists(reviewVerdictsPath));
  if (hasCuratedReviewVerdicts || !reviewVerdictsOutputPath) {
    return false;
  }

  const reviewPacket = await readExistingJson(reviewPacketJsonPath);
  if (countReviewSamples(reviewPacket) === 0) {
    return null;
  }

  return runNodeScript(
    repoRoot,
    path.join(repoRoot, 'scripts/evals/build-provisional-review-verdicts.mjs'),
    [
      '--packet',
      reviewPacketJsonPath,
      '--output-json',
      reviewVerdictsOutputPath,
      '--source-report',
      reviewPacketMarkdownPath,
      '--repo',
      manifest.repo_label ?? manifest.repo_id ?? path.basename(manifest.repo_root),
    ],
  );
}

export function buildReviewArgs(
  manifest,
  reviewPacketJsonPath,
  reviewPacketMarkdownPath,
  codexBatchPath,
  replayBatchPath,
) {
  const args = [
    '--repo-root',
    manifest.repo_root,
    '--tool',
    manifest.review_tool ?? 'check',
    '--output-json',
    reviewPacketJsonPath,
    '--output-md',
    reviewPacketMarkdownPath,
    '--limit',
    String(manifest.review_limit ?? 12),
  ];

  if (manifest.review_source === 'codex') {
    if (!codexBatchPath) {
      throw new Error('review_source "codex" requires a live batch artifact');
    }
    args.push('--codex-batch', codexBatchPath);
    return args;
  }

  if (manifest.review_source === 'replay') {
    if (!replayBatchPath) {
      throw new Error('review_source "replay" requires a replay batch artifact');
    }
    args.push('--replay-batch', replayBatchPath);
    return args;
  }

  if (codexBatchPath) {
    args.push('--codex-batch', codexBatchPath);
  }
  if (replayBatchPath) {
    args.push('--replay-batch', replayBatchPath);
  }

  return args;
}

async function pushExistingPathArg(args, flag, targetPath) {
  if (targetPath && (await pathExists(targetPath))) {
    args.push(flag, targetPath);
  }
}

export async function buildScorecardArgs({
  manifest,
  repoRootPath,
  mergedTelemetryJsonPath,
  scorecardJsonPath,
  scorecardMarkdownPath,
  codexBatchPath = null,
  replayBatchPath = null,
  defectReportPath = null,
  selectedReviewVerdictsPath = null,
  remediationReportPath = null,
  benchmarkPath = null,
}) {
  const scorecardArgs = [
    '--repo-label',
    manifest.repo_label ?? manifest.repo_id ?? path.basename(repoRootPath),
    '--session-telemetry',
    mergedTelemetryJsonPath,
    '--output-json',
    scorecardJsonPath,
    '--output-md',
    scorecardMarkdownPath,
  ];

  const optionalArtifactArgs = [
    ['--codex-batch', codexBatchPath],
    ['--replay-batch', replayBatchPath],
    ['--defect-report', defectReportPath],
    ['--remediation-report', remediationReportPath],
    ['--benchmark', benchmarkPath],
  ];

  for (const [flag, targetPath] of optionalArtifactArgs) {
    await pushExistingPathArg(scorecardArgs, flag, targetPath);
  }
  if (selectedReviewVerdictsPath) {
    scorecardArgs.push('--review-verdicts', selectedReviewVerdictsPath);
  }

  return scorecardArgs;
}

export async function readExistingJson(targetPath) {
  if (!(await pathExists(targetPath))) {
    return null;
  }

  return readJson(targetPath);
}

function buildNumericDelta(currentValue, previousValue) {
  if (!Number.isFinite(currentValue) || !Number.isFinite(previousValue)) {
    return null;
  }

  return {
    previous: previousValue,
    current: currentValue,
    delta: Number((currentValue - previousValue).toFixed(3)),
  };
}

function buildRecommendationChanges(currentScorecard, previousScorecard) {
  if (!currentScorecard || !previousScorecard) {
    return [];
  }

  const previousBySignalKind = new Map(
    (previousScorecard.signals ?? []).map((signal) => [
      signal.signal_kind,
      signal.promotion_recommendation,
    ]),
  );

  return (currentScorecard.signals ?? [])
    .map((signal) => ({
      signal_kind: signal.signal_kind,
      previous: previousBySignalKind.get(signal.signal_kind) ?? null,
      current: signal.promotion_recommendation ?? null,
    }))
    .filter((entry) => entry.previous !== null && entry.previous !== entry.current);
}

function buildReviewVerdictsMode(selectedReviewVerdictsPath, selectedReviewVerdicts) {
  if (!selectedReviewVerdictsPath) {
    return 'missing';
  }
  if (selectedReviewVerdicts?.provisional) {
    return 'provisional';
  }

  return 'curated';
}

export async function existingPathOrNull(targetPath) {
  if (targetPath && (await pathExists(targetPath))) {
    return targetPath;
  }

  return null;
}

export function buildSummaryDelta(
  currentScorecard,
  previousScorecard,
  currentBacklog,
  previousBacklog,
  currentReviewPacket,
  previousReviewPacket,
) {
  if (!previousScorecard && !previousBacklog && !previousReviewPacket) {
    return null;
  }

  return {
    total_signals: buildNumericDelta(
      currentScorecard?.summary?.total_signals ?? 0,
      previousScorecard?.summary?.total_signals ?? 0,
    ),
    weak_signal_count: buildNumericDelta(
      currentBacklog?.summary?.weak_signal_count ?? 0,
      previousBacklog?.summary?.weak_signal_count ?? 0,
    ),
    review_sample_count: buildNumericDelta(
      countReviewSamples(currentReviewPacket),
      countReviewSamples(previousReviewPacket),
    ),
    live_clean_rate: buildNumericDelta(
      currentBacklog?.summary?.live_clean_rate,
      previousBacklog?.summary?.live_clean_rate,
    ),
    replay_clean_rate: buildNumericDelta(
      currentBacklog?.summary?.replay_clean_rate,
      previousBacklog?.summary?.replay_clean_rate,
    ),
    recommended_next_signal: {
      previous: previousBacklog?.summary?.recommended_next_signal ?? null,
      current: currentBacklog?.summary?.recommended_next_signal ?? null,
      changed:
        (previousBacklog?.summary?.recommended_next_signal ?? null) !==
        (currentBacklog?.summary?.recommended_next_signal ?? null),
    },
    recommendation_changes: buildRecommendationChanges(currentScorecard, previousScorecard),
  };
}

export function buildWarnings(
  selectedReviewVerdictsPath,
  defectReportPath,
  remediationReportPath,
  benchmarkPath,
  reviewPacket,
  selectedReviewVerdicts,
) {
  const warnings = [];

  if (!selectedReviewVerdictsPath) {
    warnings.push('review verdict input missing; scorecard precision metrics have no curated review evidence');
  } else if (selectedReviewVerdicts?.provisional) {
    warnings.push('using provisional review verdicts generated from packet metadata; replace with curated review before treating precision metrics as promotion-grade evidence');
  }
  if (!defectReportPath) {
    warnings.push('seeded defect report missing; scorecard recall metrics have no deterministic detector coverage');
  }
  if (!remediationReportPath) {
    warnings.push('remediation report missing; fix-guidance quality is not grounded by repair outcomes');
  }
  if (!benchmarkPath) {
    warnings.push('benchmark artifact missing; latency metrics are unavailable');
  }
  if (countReviewSamples(reviewPacket) === 0) {
    warnings.push('review packet has zero samples; inspect capture selection or kind filters before relying on review coverage');
  }

  return warnings;
}

export function buildSummaryArtifacts({
  outputDir,
  stableReviewPacketJsonPath,
  reviewPacketJsonPath,
  reviewPacket,
  previousReviewPacketSnapshotPath,
  selectedReviewVerdictsPath,
  stableReviewVerdictsOutputPath,
  runReviewVerdictsOutputPath,
  stableScorecardJsonPath,
  scorecardJsonPath,
  previousScorecardSnapshotPath,
  stableBacklogJsonPath,
  backlogJsonPath,
  previousBacklogSnapshotPath,
  mergedTelemetryJsonPath,
  codexBatchResult,
  codexBatchOutputDir,
  replayBatchResult,
  replayBatchOutputDir,
  selectedReviewVerdicts,
  scorecard,
  backlog,
}) {
  return {
    codex_batch_json: codexBatchResult
      ? path.join(codexBatchOutputDir, 'codex-session-batch.json')
      : null,
    replay_batch_json: replayBatchResult
      ? path.join(replayBatchOutputDir, 'diff-replay-batch.json')
      : null,
    session_telemetry_json: mergedTelemetryJsonPath,
    review_packet_json: stableReviewPacketJsonPath ?? reviewPacketJsonPath,
    review_packet_run_json: reviewPacket ? reviewPacketJsonPath : null,
    previous_review_packet_json: previousReviewPacketSnapshotPath,
    review_verdicts_input: selectedReviewVerdictsPath,
    review_verdicts_output:
      stableReviewVerdictsOutputPath ?? runReviewVerdictsOutputPath,
    review_verdicts_run_output: runReviewVerdictsOutputPath,
    review_verdicts_mode: buildReviewVerdictsMode(
      selectedReviewVerdictsPath,
      selectedReviewVerdicts,
    ),
    scorecard_json: stableScorecardJsonPath ?? scorecardJsonPath,
    scorecard_run_json: scorecard ? scorecardJsonPath : null,
    previous_scorecard_json: previousScorecardSnapshotPath,
    backlog_json: stableBacklogJsonPath ?? backlogJsonPath,
    backlog_run_json: backlog ? backlogJsonPath : null,
    previous_backlog_json: previousBacklogSnapshotPath,
  };
}
