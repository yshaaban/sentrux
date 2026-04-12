#!/usr/bin/env node

import { access, copyFile, mkdir, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { execFile as execFileCallback } from 'node:child_process';
import { promisify } from 'node:util';
import {
  defaultBatchOutputDir,
  readJson,
  writeJson,
  writeText,
} from '../lib/eval-batch.mjs';
import {
  formatSessionTelemetrySummaryMarkdown,
  mergeSessionTelemetrySummaries,
} from '../lib/session-telemetry.mjs';

const execFile = promisify(execFileCallback);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

export function parseArgs(argv) {
  const result = {
    manifestPath: null,
    outputDir: null,
    skipLive: false,
    skipReplay: false,
    skipReview: false,
    skipScorecard: false,
    skipBacklog: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === '--manifest') {
      index += 1;
      result.manifestPath = argv[index];
      continue;
    }
    if (value === '--output-dir') {
      index += 1;
      result.outputDir = argv[index];
      continue;
    }
    if (value === '--skip-live') {
      result.skipLive = true;
      continue;
    }
    if (value === '--skip-replay') {
      result.skipReplay = true;
      continue;
    }
    if (value === '--skip-review') {
      result.skipReview = true;
      continue;
    }
    if (value === '--skip-scorecard') {
      result.skipScorecard = true;
      continue;
    }
    if (value === '--skip-backlog') {
      result.skipBacklog = true;
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }

  if (!result.manifestPath) {
    throw new Error('Missing required --manifest path');
  }

  return result;
}

function nowIso() {
  return new Date().toISOString();
}

async function pathExists(targetPath) {
  if (!targetPath) {
    return false;
  }

  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}

async function pushExistingPathArg(args, flag, targetPath) {
  if (targetPath && (await pathExists(targetPath))) {
    args.push(flag, targetPath);
  }
}

async function loadRepoCalibrationManifest(manifestPath) {
  const manifest = await readJson(manifestPath);
  if (manifest?.schema_version !== 1) {
    throw new Error(`Unsupported repo calibration manifest: ${manifestPath}`);
  }

  return manifest;
}

function normalizeExpectedSignalKinds(expectedSignalKinds) {
  return [...new Set((expectedSignalKinds ?? []).filter(Boolean))].sort();
}

function resolveManifestPath(manifestDir, relativePath) {
  if (!relativePath) {
    return null;
  }

  return path.resolve(manifestDir, relativePath);
}

function resolveRepoArtifactPath(repoRootPath, relativePath) {
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

async function acquireLoopLock(lockPath, metadata) {
  try {
    await mkdir(lockPath);
  } catch (error) {
    if (error && typeof error === 'object' && error.code === 'EEXIST') {
      throw new Error(`Another calibration loop already holds the repo lock: ${lockPath}`);
    }
    throw error;
  }

  await writeFile(path.join(lockPath, 'owner.json'), `${JSON.stringify(metadata, null, 2)}
`, 'utf8');
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

async function publishArtifacts(pairs) {
  for (const pair of pairs) {
    if (!pair?.sourcePath || !pair?.targetPath) {
      continue;
    }

    await publishArtifact(pair.sourcePath, pair.targetPath);
  }
}

function buildBatchRunArgs(manifestPath, outputDir, cohortManifestPath, cohortId) {
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

async function runNodeScript(scriptPath, args) {
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

function buildSummaryMarkdown(summary) {
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

function buildBatchExpectationWarnings(batchManifest, batchResult, idKey, laneLabel) {
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

function buildBatchFailureWarnings(batchResult, laneLabel) {
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

async function maybeBuildProvisionalReviewVerdicts(
  manifest,
  reviewPacketJsonPath,
  reviewPacketMarkdownPath,
  reviewVerdictsOutputPath,
  reviewVerdictsPath,
) {
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

  return runNodeScript(path.join(repoRoot, 'scripts/evals/build-provisional-review-verdicts.mjs'), [
    '--packet',
    reviewPacketJsonPath,
    '--output-json',
    reviewVerdictsOutputPath,
    '--source-report',
    reviewPacketMarkdownPath,
    '--repo',
    manifest.repo_label ?? manifest.repo_id ?? path.basename(manifest.repo_root),
  ]);
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

async function readExistingJson(targetPath) {
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

function countReviewSamples(reviewPacket) {
  return reviewPacket?.summary?.sample_count ?? reviewPacket?.samples?.length ?? 0;
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

async function existingPathOrNull(targetPath) {
  if (targetPath && (await pathExists(targetPath))) {
    return targetPath;
  }

  return null;
}

function buildSummaryDelta(currentScorecard, previousScorecard, currentBacklog, previousBacklog, currentReviewPacket, previousReviewPacket) {
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

function buildWarnings(
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

async function main() {
  const args = parseArgs(process.argv);
  const manifestPath = path.resolve(args.manifestPath);
  const manifestDir = path.dirname(manifestPath);
  const manifest = await loadRepoCalibrationManifest(manifestPath);
  const repoRootPath = path.resolve(manifest.repo_root);
  const outputDir = path.resolve(
    args.outputDir ??
      defaultBatchOutputDir(
        repoRootPath,
        'repo-calibration-loop',
        manifest.repo_id ?? path.basename(repoRootPath),
      ),
  );
  const lockPath = path.join(
    repoRootPath,
    '.sentrux',
    'evals',
    `.repo-calibration-${manifest.repo_id ?? path.basename(repoRootPath)}.lock`,
  );
  const releaseLoopLock = await acquireLoopLock(lockPath, {
    repo_id: manifest.repo_id ?? path.basename(repoRootPath),
    repo_root: repoRootPath,
    output_dir: outputDir,
    pid: process.pid,
    started_at: nowIso(),
    manifest_path: manifestPath,
  });

  try {
    const cohortManifestPath = resolveManifestPath(manifestDir, manifest.cohort_manifest);
    const codexBatchManifestPath = resolveManifestPath(
      manifestDir,
      manifest.live_batch_manifest ?? manifest.codex_batch_manifest,
    );
    const replayBatchManifestPath = resolveManifestPath(
      manifestDir,
      manifest.replay_batch_manifest,
    );
    const codexBatchManifest =
      codexBatchManifestPath && (await pathExists(codexBatchManifestPath))
        ? await readJson(codexBatchManifestPath)
        : null;
    const replayBatchManifest =
      replayBatchManifestPath && (await pathExists(replayBatchManifestPath))
        ? await readJson(replayBatchManifestPath)
        : null;
    const artifactConfig = manifest.artifacts ?? {};

    const stableReviewVerdictsOutputPath = resolveRepoArtifactPath(
      repoRootPath,
      artifactConfig.review_verdicts_output,
    );
    const reviewVerdictsPath = resolveManifestPath(
      manifestDir,
      artifactConfig.review_verdicts_input ?? manifest.review_verdicts,
    );
    const defectReportPath = resolveManifestPath(
      manifestDir,
      artifactConfig.seeded_defect_report ?? manifest.defect_report,
    );
    const remediationReportPath = resolveManifestPath(
      manifestDir,
      artifactConfig.remediation_report ?? manifest.remediation_report,
    );
    const benchmarkPath = resolveManifestPath(
      manifestDir,
      artifactConfig.benchmark_artifact ?? manifest.benchmark_artifact,
    );

    const codexBatchOutputDir = path.join(outputDir, 'codex-batch');
    const replayBatchOutputDir = path.join(outputDir, 'replay-batch');
    const configuredReviewPacketPath = resolveRepoArtifactPath(
      repoRootPath,
      artifactConfig.review_packet_output,
    );
    const stableReviewPacketJsonPath = deriveCompanionPath(
      configuredReviewPacketPath,
      '.json',
    );
    const stableReviewPacketMarkdownPath = deriveCompanionPath(
      configuredReviewPacketPath,
      '.md',
    );
    const reviewPacketJsonPath = buildRunArtifactPath(
      outputDir,
      'check-review-packet',
      '.json',
    );
    const reviewPacketMarkdownPath = buildRunArtifactPath(
      outputDir,
      'check-review-packet',
      '.md',
    );
    const runReviewVerdictsOutputPath = buildRunArtifactPath(
      outputDir,
      'review-verdicts',
      '.json',
    );
    const mergedTelemetryJsonPath = path.join(
      outputDir,
      'session-telemetry-summary.json',
    );
    const mergedTelemetryMarkdownPath = path.join(
      outputDir,
      'session-telemetry-summary.md',
    );
    const configuredScorecardPath = resolveRepoArtifactPath(
      repoRootPath,
      artifactConfig.scorecard_output,
    );
    const configuredBacklogPath = resolveRepoArtifactPath(
      repoRootPath,
      artifactConfig.backlog_output,
    );
    const stableScorecardJsonPath = deriveCompanionPath(
      configuredScorecardPath,
      '.json',
    );
    const stableScorecardMarkdownPath = deriveCompanionPath(
      configuredScorecardPath,
      '.md',
    );
    const stableBacklogJsonPath = deriveCompanionPath(
      configuredBacklogPath,
      '.json',
    );
    const stableBacklogMarkdownPath = deriveCompanionPath(
      configuredBacklogPath,
      '.md',
    );
    const scorecardJsonPath = buildRunArtifactPath(
      outputDir,
      'signal-scorecard',
      '.json',
    );
    const scorecardMarkdownPath = buildRunArtifactPath(
      outputDir,
      'signal-scorecard',
      '.md',
    );
    const backlogJsonPath = buildRunArtifactPath(
      outputDir,
      'signal-backlog',
      '.json',
    );
    const backlogMarkdownPath = buildRunArtifactPath(
      outputDir,
      'signal-backlog',
      '.md',
    );

    const previousReviewPacket = await readExistingJson(stableReviewPacketJsonPath);
    const previousScorecard = await readExistingJson(stableScorecardJsonPath);
    const previousBacklog = await readExistingJson(stableBacklogJsonPath);
    const previousReviewPacketSnapshotPath = previousReviewPacket
      ? path.join(outputDir, 'previous-check-review-packet.json')
      : null;
    const previousScorecardSnapshotPath = previousScorecard
      ? path.join(outputDir, 'previous-signal-scorecard.json')
      : null;
    const previousBacklogSnapshotPath = previousBacklog
      ? path.join(outputDir, 'previous-signal-backlog.json')
      : null;

    if (previousReviewPacketSnapshotPath) {
      await writeJson(previousReviewPacketSnapshotPath, previousReviewPacket);
    }
    if (previousScorecardSnapshotPath) {
      await writeJson(previousScorecardSnapshotPath, previousScorecard);
    }
    if (previousBacklogSnapshotPath) {
      await writeJson(previousBacklogSnapshotPath, previousBacklog);
    }

    const runs = [];
    let codexBatchResult = null;
    let replayBatchResult = null;
    let selectedReviewVerdictsPath = null;

    if (!args.skipLive && codexBatchManifestPath) {
      const liveArgs = buildBatchRunArgs(
        codexBatchManifestPath,
        codexBatchOutputDir,
        cohortManifestPath,
        manifest.cohort_id,
      );
      runs.push(
        await runNodeScript(
          path.join(repoRoot, 'scripts/evals/run-codex-session-batch.mjs'),
          liveArgs,
        ),
      );
      codexBatchResult = await readJson(
        path.join(codexBatchOutputDir, 'codex-session-batch.json'),
      );
    }

    if (!args.skipReplay && replayBatchManifestPath) {
      const replayArgs = buildBatchRunArgs(
        replayBatchManifestPath,
        replayBatchOutputDir,
        cohortManifestPath,
        manifest.cohort_id,
      );
      runs.push(
        await runNodeScript(
          path.join(repoRoot, 'scripts/evals/run-diff-replay-batch.mjs'),
          replayArgs,
        ),
      );
      replayBatchResult = await readJson(
        path.join(replayBatchOutputDir, 'diff-replay-batch.json'),
      );
    }

    const telemetrySummaries = [
      codexBatchResult?.telemetry_summary ?? null,
      replayBatchResult?.telemetry_summary ?? null,
    ].filter(Boolean);
    const mergedTelemetry = mergeSessionTelemetrySummaries(telemetrySummaries, {
      repoRoot: repoRootPath,
      sourcePaths: telemetrySummaries.flatMap((summary) => summary.source_paths ?? []),
    });

    await writeJson(mergedTelemetryJsonPath, mergedTelemetry);
    await writeText(
      mergedTelemetryMarkdownPath,
      formatSessionTelemetrySummaryMarkdown(mergedTelemetry),
    );

    if (!args.skipReview && (codexBatchResult || replayBatchResult)) {
      const reviewArgs = buildReviewArgs(
        manifest,
        reviewPacketJsonPath,
        reviewPacketMarkdownPath,
        codexBatchResult
          ? path.join(codexBatchOutputDir, 'codex-session-batch.json')
          : null,
        replayBatchResult
          ? path.join(replayBatchOutputDir, 'diff-replay-batch.json')
          : null,
      );

      runs.push(
        await runNodeScript(
          path.join(repoRoot, 'scripts/evals/build-check-review-packet.mjs'),
          reviewArgs,
        ),
      );
    }

    if (!args.skipReview) {
      const provisionalVerdictRun = await maybeBuildProvisionalReviewVerdicts(
        manifest,
        reviewPacketJsonPath,
        reviewPacketMarkdownPath,
        runReviewVerdictsOutputPath,
        reviewVerdictsPath,
      );
      if (provisionalVerdictRun) {
        runs.push(provisionalVerdictRun);
      }
    }

    if (!args.skipScorecard) {
      selectedReviewVerdictsPath = await selectReviewVerdictsPath(
        runReviewVerdictsOutputPath,
        reviewVerdictsPath,
      );

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

      await pushExistingPathArg(scorecardArgs, '--codex-batch', codexBatchResultPath);
      await pushExistingPathArg(scorecardArgs, '--replay-batch', replayBatchResultPath);
      await pushExistingPathArg(scorecardArgs, '--defect-report', defectReportPath);
      if (selectedReviewVerdictsPath) {
        scorecardArgs.push('--review-verdicts', selectedReviewVerdictsPath);
      }
      await pushExistingPathArg(scorecardArgs, '--remediation-report', remediationReportPath);
      await pushExistingPathArg(scorecardArgs, '--benchmark', benchmarkPath);

      runs.push(
        await runNodeScript(
          path.join(repoRoot, 'scripts/evals/build-signal-scorecard.mjs'),
          scorecardArgs,
        ),
      );
    }

    if (!args.skipBacklog) {
      if (!(await pathExists(scorecardJsonPath))) {
        throw new Error(
          `Cannot build backlog without a scorecard artifact: ${scorecardJsonPath}`,
        );
      }
      const backlogArgs = [
        '--scorecard',
        scorecardJsonPath,
        '--output-json',
        backlogJsonPath,
        '--output-md',
        backlogMarkdownPath,
      ];

      if (cohortManifestPath) {
        backlogArgs.push('--cohort-manifest', cohortManifestPath);
      }
      if (manifest.cohort_id) {
        backlogArgs.push('--cohort-id', manifest.cohort_id);
      }
      if (codexBatchResult) {
        backlogArgs.push(
          '--codex-batch',
          path.join(codexBatchOutputDir, 'codex-session-batch.json'),
        );
      }
      if (replayBatchResult) {
        backlogArgs.push(
          '--replay-batch',
          path.join(replayBatchOutputDir, 'diff-replay-batch.json'),
        );
      }

      runs.push(
        await runNodeScript(
          path.join(repoRoot, 'scripts/evals/build-signal-backlog.mjs'),
          backlogArgs,
        ),
      );
    }

    const reviewPacket = (await pathExists(reviewPacketJsonPath))
      ? await readJson(reviewPacketJsonPath)
      : null;
    const selectedReviewVerdicts = selectedReviewVerdictsPath
      ? await readExistingJson(selectedReviewVerdictsPath)
      : null;
    const scorecard = (await pathExists(scorecardJsonPath))
      ? await readJson(scorecardJsonPath)
      : null;
    const backlog = (await pathExists(backlogJsonPath))
      ? await readJson(backlogJsonPath)
      : null;

    const summary = {
      schema_version: 1,
      generated_at: nowIso(),
      repo_id: manifest.repo_id ?? path.basename(repoRootPath),
      repo_label: manifest.repo_label ?? manifest.repo_id ?? path.basename(repoRootPath),
      repo_root: repoRootPath,
      output_dir: outputDir,
      cohort_id:
        manifest.cohort_id ??
        codexBatchResult?.cohort_id ??
        replayBatchResult?.cohort_id ??
        null,
      artifacts: {
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
      },
      summary: {
        session_count: mergedTelemetry.summary.session_count ?? 0,
        total_signals: scorecard?.summary?.total_signals ?? 0,
        weak_signal_count: backlog?.summary?.weak_signal_count ?? 0,
        review_sample_count: countReviewSamples(reviewPacket),
        live_clean_rate: backlog?.summary?.live_clean_rate ?? null,
        replay_clean_rate: backlog?.summary?.replay_clean_rate ?? null,
        recommended_next_signal: backlog?.summary?.recommended_next_signal ?? null,
        live_failure_count: codexBatchResult?.failure_count ?? 0,
        replay_failure_count: replayBatchResult?.failure_count ?? 0,
      },
      delta: buildSummaryDelta(
        scorecard,
        previousScorecard,
        backlog,
        previousBacklog,
        reviewPacket,
        previousReviewPacket,
      ),
      warnings: [
        ...buildWarnings(
          selectedReviewVerdictsPath,
          await existingPathOrNull(defectReportPath),
          await existingPathOrNull(remediationReportPath),
          await existingPathOrNull(benchmarkPath),
          reviewPacket,
          selectedReviewVerdicts,
        ),
        ...buildBatchExpectationWarnings(
          codexBatchManifest,
          codexBatchResult,
          'task_id',
          'live',
        ),
        ...buildBatchFailureWarnings(codexBatchResult, 'live'),
        ...buildBatchExpectationWarnings(
          replayBatchManifest,
          replayBatchResult,
          'replay_id',
          'replay',
        ),
        ...buildBatchFailureWarnings(replayBatchResult, 'replay'),
      ],
      runs,
    };

    await publishArtifacts([
      {
        sourcePath: reviewPacketJsonPath,
        targetPath: stableReviewPacketJsonPath,
      },
      {
        sourcePath: reviewPacketMarkdownPath,
        targetPath: stableReviewPacketMarkdownPath,
      },
      {
        sourcePath: selectedReviewVerdictsPath,
        targetPath: stableReviewVerdictsOutputPath,
      },
      {
        sourcePath: scorecardJsonPath,
        targetPath: stableScorecardJsonPath,
      },
      {
        sourcePath: scorecardMarkdownPath,
        targetPath: stableScorecardMarkdownPath,
      },
      {
        sourcePath: backlogJsonPath,
        targetPath: stableBacklogJsonPath,
      },
      {
        sourcePath: backlogMarkdownPath,
        targetPath: stableBacklogMarkdownPath,
      },
    ]);

    const latestPointerPath = path.join(
      path.dirname(
        stableScorecardJsonPath ??
          stableBacklogJsonPath ??
          stableReviewPacketJsonPath ??
          outputDir,
      ),
      'latest.json',
    );
    await writeJson(path.join(outputDir, 'repo-calibration-loop.json'), summary);
    await writeText(
      path.join(outputDir, 'repo-calibration-loop.md'),
      buildSummaryMarkdown(summary),
    );
    await writeJson(latestPointerPath, {
      repo_id: summary.repo_id,
      generated_at: summary.generated_at,
      latest_output_dir: outputDir,
      summary_json: path.join(outputDir, 'repo-calibration-loop.json'),
      scorecard_json: summary.artifacts.scorecard_json,
      backlog_json: summary.artifacts.backlog_json,
      review_packet_json: summary.artifacts.review_packet_json,
    });

    console.log(
      `Completed repo calibration loop for ${summary.repo_label}. Artifacts written to ${outputDir}`,
    );
  } finally {
    await releaseLoopLock();
  }
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
