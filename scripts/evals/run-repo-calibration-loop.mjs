#!/usr/bin/env node

import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
  defaultBatchOutputDir,
  readJson,
  resolveManifestPath,
  writeJson,
  writeText,
} from '../lib/eval-batch.mjs';
import { parseCliArgs } from '../lib/eval-support.mjs';
import { pathExists } from '../lib/repo-calibration-artifacts.mjs';
import {
  acquireLoopLock,
  buildBatchExpectationWarnings,
  buildBatchFailureWarnings,
  buildBatchRunArgs,
  buildReviewArgs,
  buildScorecardArgs,
  buildSummaryArtifacts,
  buildSummaryDelta,
  buildSummaryMarkdown,
  buildWarnings,
  existingPathOrNull,
  maybeBuildProvisionalReviewVerdicts,
  publishArtifacts,
  readExistingJson,
  resolveLoopArtifactPaths,
  runNodeScript,
  selectReviewVerdictsPath,
} from '../lib/repo-calibration-loop-support.mjs';
import {
  formatSessionTelemetrySummaryMarkdown,
  mergeSessionTelemetrySummaries,
} from '../lib/session-telemetry.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

export {
  buildReviewArgs,
  buildScorecardArgs,
  selectReviewVerdictsPath,
} from '../lib/repo-calibration-loop-support.mjs';

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

  parseCliArgs(argv, result, {
    flags: {
      '--skip-live': function setSkipLive(target) {
        target.skipLive = true;
      },
      '--skip-replay': function setSkipReplay(target) {
        target.skipReplay = true;
      },
      '--skip-review': function setSkipReview(target) {
        target.skipReview = true;
      },
      '--skip-scorecard': function setSkipScorecard(target) {
        target.skipScorecard = true;
      },
      '--skip-backlog': function setSkipBacklog(target) {
        target.skipBacklog = true;
      },
    },
    values: {
      '--manifest': function setManifestPath(target, value) {
        target.manifestPath = value;
      },
      '--output-dir': function setOutputDir(target, value) {
        target.outputDir = value;
      },
    },
  });

  if (!result.manifestPath) {
    throw new Error('Missing required --manifest path');
  }

  return result;
}

function nowIso() {
  return new Date().toISOString();
}

async function loadRepoCalibrationManifest(manifestPath) {
  const manifest = await readJson(manifestPath);
  if (manifest?.schema_version !== 1) {
    throw new Error(`Unsupported repo calibration manifest: ${manifestPath}`);
  }

  return manifest;
}

async function writeSnapshotIfPresent(targetPath, value) {
  if (!targetPath || !value) {
    return;
  }

  await writeJson(targetPath, value);
}

async function runBatchLane({
  skip,
  manifestPath,
  outputDir,
  cohortManifestPath,
  cohortId,
  scriptName,
  resultPath,
  runs,
}) {
  if (skip || !manifestPath) {
    return null;
  }

  const laneArgs = buildBatchRunArgs(
    manifestPath,
    outputDir,
    cohortManifestPath,
    cohortId,
  );
  runs.push(
    await runNodeScript(
      repoRoot,
      path.join(repoRoot, scriptName),
      laneArgs,
    ),
  );
  return readJson(resultPath);
}

async function writeMergedTelemetry(outputPaths, telemetrySummaries, repoRootPath) {
  const mergedTelemetry = mergeSessionTelemetrySummaries(telemetrySummaries, {
    repoRoot: repoRootPath,
    sourcePaths: telemetrySummaries.flatMap((summary) => summary.source_paths ?? []),
  });

  await writeJson(outputPaths.mergedTelemetryJsonPath, mergedTelemetry);
  await writeText(
    outputPaths.mergedTelemetryMarkdownPath,
    formatSessionTelemetrySummaryMarkdown(mergedTelemetry),
  );

  return mergedTelemetry;
}

async function runReviewStage({
  skipReview,
  manifest,
  reviewPacketJsonPath,
  reviewPacketMarkdownPath,
  codexBatchResult,
  codexBatchResultPath,
  replayBatchResult,
  replayBatchResultPath,
  runs,
}) {
  if (skipReview || (!codexBatchResult && !replayBatchResult)) {
    return;
  }

  const reviewArgs = buildReviewArgs(
    manifest,
    reviewPacketJsonPath,
    reviewPacketMarkdownPath,
    codexBatchResult ? codexBatchResultPath : null,
    replayBatchResult ? replayBatchResultPath : null,
  );

  runs.push(
    await runNodeScript(
      repoRoot,
      path.join(repoRoot, 'scripts/evals/build-check-review-packet.mjs'),
      reviewArgs,
    ),
  );
}

async function runScorecardStage({
  skipScorecard,
  manifest,
  repoRootPath,
  mergedTelemetryJsonPath,
  scorecardJsonPath,
  scorecardMarkdownPath,
  codexBatchResult,
  codexBatchResultPath,
  replayBatchResult,
  replayBatchResultPath,
  defectReportPath,
  selectedReviewVerdictsPath,
  remediationReportPath,
  benchmarkPath,
  runs,
}) {
  if (skipScorecard) {
    return;
  }

  const scorecardArgs = await buildScorecardArgs({
    manifest,
    repoRootPath,
    mergedTelemetryJsonPath,
    scorecardJsonPath,
    scorecardMarkdownPath,
    codexBatchPath: codexBatchResult ? codexBatchResultPath : null,
    replayBatchPath: replayBatchResult ? replayBatchResultPath : null,
    defectReportPath,
    selectedReviewVerdictsPath,
    remediationReportPath,
    benchmarkPath,
  });

  runs.push(
    await runNodeScript(
      repoRoot,
      path.join(repoRoot, 'scripts/evals/build-signal-scorecard.mjs'),
      scorecardArgs,
    ),
  );
}

async function runBacklogStage({
  skipBacklog,
  scorecardJsonPath,
  backlogJsonPath,
  backlogMarkdownPath,
  cohortManifestPath,
  cohortId,
  codexBatchResult,
  codexBatchResultPath,
  replayBatchResult,
  replayBatchResultPath,
  runs,
}) {
  if (skipBacklog) {
    return;
  }
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
  if (cohortId) {
    backlogArgs.push('--cohort-id', cohortId);
  }
  if (codexBatchResult) {
    backlogArgs.push('--codex-batch', codexBatchResultPath);
  }
  if (replayBatchResult) {
    backlogArgs.push('--replay-batch', replayBatchResultPath);
  }

  runs.push(
    await runNodeScript(
      repoRoot,
      path.join(repoRoot, 'scripts/evals/build-signal-backlog.mjs'),
      backlogArgs,
    ),
  );
}

async function loadBatchManifestIfPresent(targetPath) {
  if (!targetPath || !(await pathExists(targetPath))) {
    return null;
  }

  return readJson(targetPath);
}

async function loadLoopBatchManifests(paths) {
  return {
    codexBatchManifest: await loadBatchManifestIfPresent(paths.codexBatchManifestPath),
    replayBatchManifest: await loadBatchManifestIfPresent(paths.replayBatchManifestPath),
  };
}

function buildPreviousSnapshotPath(outputDir, baseName, artifact) {
  return artifact ? path.join(outputDir, `${baseName}.json`) : null;
}

async function capturePreviousArtifacts(outputDir, paths) {
  const previousReviewPacket = await readExistingJson(paths.stableReviewPacketJsonPath);
  const previousScorecard = await readExistingJson(paths.stableScorecardJsonPath);
  const previousBacklog = await readExistingJson(paths.stableBacklogJsonPath);
  const previousReviewPacketSnapshotPath = buildPreviousSnapshotPath(
    outputDir,
    'previous-check-review-packet',
    previousReviewPacket,
  );
  const previousScorecardSnapshotPath = buildPreviousSnapshotPath(
    outputDir,
    'previous-signal-scorecard',
    previousScorecard,
  );
  const previousBacklogSnapshotPath = buildPreviousSnapshotPath(
    outputDir,
    'previous-signal-backlog',
    previousBacklog,
  );

  await writeSnapshotIfPresent(previousReviewPacketSnapshotPath, previousReviewPacket);
  await writeSnapshotIfPresent(previousScorecardSnapshotPath, previousScorecard);
  await writeSnapshotIfPresent(previousBacklogSnapshotPath, previousBacklog);

  return {
    previousReviewPacket,
    previousScorecard,
    previousBacklog,
    previousReviewPacketSnapshotPath,
    previousScorecardSnapshotPath,
    previousBacklogSnapshotPath,
  };
}

async function loadGeneratedArtifacts(paths, selectedReviewVerdictsPath) {
  const reviewPacket = await loadBatchManifestIfPresent(paths.reviewPacketJsonPath);
  const scorecard = await loadBatchManifestIfPresent(paths.scorecardJsonPath);
  const backlog = await loadBatchManifestIfPresent(paths.backlogJsonPath);
  const selectedReviewVerdicts = selectedReviewVerdictsPath
    ? await readExistingJson(selectedReviewVerdictsPath)
    : null;

  return {
    reviewPacket,
    selectedReviewVerdicts,
    scorecard,
    backlog,
  };
}

async function buildLoopWarningSet({
  paths,
  reviewPacket,
  selectedReviewVerdicts,
  selectedReviewVerdictsPath,
  manifests,
  batchResults,
}) {
  return [
    ...buildWarnings(
      selectedReviewVerdictsPath,
      await existingPathOrNull(paths.defectReportPath),
      await existingPathOrNull(paths.remediationReportPath),
      await existingPathOrNull(paths.benchmarkPath),
      reviewPacket,
      selectedReviewVerdicts,
    ),
    ...buildBatchExpectationWarnings(
      manifests.codexBatchManifest,
      batchResults.codexBatchResult,
      'task_id',
      'live',
    ),
    ...buildBatchFailureWarnings(batchResults.codexBatchResult, 'live'),
    ...buildBatchExpectationWarnings(
      manifests.replayBatchManifest,
      batchResults.replayBatchResult,
      'replay_id',
      'replay',
    ),
    ...buildBatchFailureWarnings(batchResults.replayBatchResult, 'replay'),
  ];
}

function buildLoopSummary({
  outputDir,
  repoId,
  repoRootPath,
  manifest,
  mergedTelemetry,
  reviewPacket,
  selectedReviewVerdicts,
  scorecard,
  backlog,
  previousArtifacts,
  stableArtifacts,
  batchResults,
  warnings,
  runs,
}) {
  return {
    schema_version: 1,
    generated_at: nowIso(),
    repo_id: repoId,
    repo_label: manifest.repo_label ?? repoId,
    repo_root: repoRootPath,
    output_dir: outputDir,
    cohort_id:
      manifest.cohort_id ??
      batchResults.codexBatchResult?.cohort_id ??
      batchResults.replayBatchResult?.cohort_id ??
      null,
    artifacts: stableArtifacts,
    summary: {
      session_count: mergedTelemetry.summary.session_count ?? 0,
      total_signals: scorecard?.summary?.total_signals ?? 0,
      weak_signal_count: backlog?.summary?.weak_signal_count ?? 0,
      review_sample_count:
        reviewPacket?.summary?.sample_count ?? reviewPacket?.samples?.length ?? 0,
      live_clean_rate: backlog?.summary?.live_clean_rate ?? null,
      replay_clean_rate: backlog?.summary?.replay_clean_rate ?? null,
      recommended_next_signal: backlog?.summary?.recommended_next_signal ?? null,
      live_failure_count: batchResults.codexBatchResult?.failure_count ?? 0,
      replay_failure_count: batchResults.replayBatchResult?.failure_count ?? 0,
    },
    delta: buildSummaryDelta(
      scorecard,
      previousArtifacts.previousScorecard,
      backlog,
      previousArtifacts.previousBacklog,
      reviewPacket,
      previousArtifacts.previousReviewPacket,
    ),
    warnings,
    runs,
  };
}

async function publishStableLoopArtifacts(paths, selectedReviewVerdictsPath) {
  await publishArtifacts([
    {
      sourcePath: paths.reviewPacketJsonPath,
      targetPath: paths.stableReviewPacketJsonPath,
    },
    {
      sourcePath: paths.reviewPacketMarkdownPath,
      targetPath: paths.stableReviewPacketMarkdownPath,
    },
    {
      sourcePath: selectedReviewVerdictsPath,
      targetPath: paths.stableReviewVerdictsOutputPath,
    },
    {
      sourcePath: paths.scorecardJsonPath,
      targetPath: paths.stableScorecardJsonPath,
    },
    {
      sourcePath: paths.scorecardMarkdownPath,
      targetPath: paths.stableScorecardMarkdownPath,
    },
    {
      sourcePath: paths.backlogJsonPath,
      targetPath: paths.stableBacklogJsonPath,
    },
    {
      sourcePath: paths.backlogMarkdownPath,
      targetPath: paths.stableBacklogMarkdownPath,
    },
  ]);
}

function buildLatestPointerPath(stableArtifacts, outputDir) {
  return path.join(
    path.dirname(
      stableArtifacts.scorecard_json ??
        stableArtifacts.backlog_json ??
        stableArtifacts.review_packet_json ??
        outputDir,
    ),
    'latest.json',
  );
}

async function writeLoopOutputs(outputDir, summary) {
  const summaryJsonPath = path.join(outputDir, 'repo-calibration-loop.json');
  const latestPointerPath = buildLatestPointerPath(summary.artifacts, outputDir);

  await writeJson(summaryJsonPath, summary);
  await writeText(
    path.join(outputDir, 'repo-calibration-loop.md'),
    buildSummaryMarkdown(summary),
  );
  await writeJson(latestPointerPath, {
    repo_id: summary.repo_id,
    generated_at: summary.generated_at,
    latest_output_dir: outputDir,
    summary_json: summaryJsonPath,
    scorecard_json: summary.artifacts.scorecard_json,
    backlog_json: summary.artifacts.backlog_json,
    review_packet_json: summary.artifacts.review_packet_json,
  });
}

async function maybeRunProvisionalVerdictStage(args, manifest, paths, runs) {
  if (args.skipReview) {
    return;
  }

  const provisionalVerdictRun = await maybeBuildProvisionalReviewVerdicts({
    repoRoot,
    manifest,
    reviewPacketJsonPath: paths.reviewPacketJsonPath,
    reviewPacketMarkdownPath: paths.reviewPacketMarkdownPath,
    reviewVerdictsOutputPath: paths.runReviewVerdictsOutputPath,
    reviewVerdictsPath: paths.reviewVerdictsPath,
  });
  if (provisionalVerdictRun) {
    runs.push(provisionalVerdictRun);
  }
}

async function resolveSelectedReviewVerdictsPath(args, paths) {
  if (args.skipScorecard) {
    return null;
  }

  return selectReviewVerdictsPath(
    paths.runReviewVerdictsOutputPath,
    paths.reviewVerdictsPath,
  );
}

async function runLoopStages({ args, manifest, paths, repoRootPath }) {
  const runs = [];
  const codexBatchResult = await runBatchLane({
    skip: args.skipLive,
    manifestPath: paths.codexBatchManifestPath,
    outputDir: paths.codexBatchOutputDir,
    cohortManifestPath: paths.cohortManifestPath,
    cohortId: manifest.cohort_id,
    scriptName: 'scripts/evals/run-codex-session-batch.mjs',
    resultPath: paths.codexBatchResultPath,
    runs,
  });
  const replayBatchResult = await runBatchLane({
    skip: args.skipReplay,
    manifestPath: paths.replayBatchManifestPath,
    outputDir: paths.replayBatchOutputDir,
    cohortManifestPath: paths.cohortManifestPath,
    cohortId: manifest.cohort_id,
    scriptName: 'scripts/evals/run-diff-replay-batch.mjs',
    resultPath: paths.replayBatchResultPath,
    runs,
  });
  const batchResults = { codexBatchResult, replayBatchResult };
  const telemetrySummaries = [
    codexBatchResult?.telemetry_summary ?? null,
    replayBatchResult?.telemetry_summary ?? null,
  ].filter(Boolean);
  const mergedTelemetry = await writeMergedTelemetry(
    {
      mergedTelemetryJsonPath: paths.mergedTelemetryJsonPath,
      mergedTelemetryMarkdownPath: paths.mergedTelemetryMarkdownPath,
    },
    telemetrySummaries,
    repoRootPath,
  );

  await runReviewStage({
    skipReview: args.skipReview,
    manifest,
    reviewPacketJsonPath: paths.reviewPacketJsonPath,
    reviewPacketMarkdownPath: paths.reviewPacketMarkdownPath,
    codexBatchResult,
    codexBatchResultPath: paths.codexBatchResultPath,
    replayBatchResult,
    replayBatchResultPath: paths.replayBatchResultPath,
    runs,
  });

  await maybeRunProvisionalVerdictStage(args, manifest, paths, runs);
  const selectedReviewVerdictsPath = await resolveSelectedReviewVerdictsPath(args, paths);

  await runScorecardStage({
    skipScorecard: args.skipScorecard,
    manifest,
    repoRootPath,
    mergedTelemetryJsonPath: paths.mergedTelemetryJsonPath,
    scorecardJsonPath: paths.scorecardJsonPath,
    scorecardMarkdownPath: paths.scorecardMarkdownPath,
    codexBatchResult,
    codexBatchResultPath: paths.codexBatchResultPath,
    replayBatchResult,
    replayBatchResultPath: paths.replayBatchResultPath,
    defectReportPath: paths.defectReportPath,
    selectedReviewVerdictsPath,
    remediationReportPath: paths.remediationReportPath,
    benchmarkPath: paths.benchmarkPath,
    runs,
  });
  await runBacklogStage({
    skipBacklog: args.skipBacklog,
    scorecardJsonPath: paths.scorecardJsonPath,
    backlogJsonPath: paths.backlogJsonPath,
    backlogMarkdownPath: paths.backlogMarkdownPath,
    cohortManifestPath: paths.cohortManifestPath,
    cohortId: manifest.cohort_id,
    codexBatchResult,
    codexBatchResultPath: paths.codexBatchResultPath,
    replayBatchResult,
    replayBatchResultPath: paths.replayBatchResultPath,
    runs,
  });

  return {
    runs,
    batchResults,
    mergedTelemetry,
    selectedReviewVerdictsPath,
    generatedArtifacts: await loadGeneratedArtifacts(paths, selectedReviewVerdictsPath),
  };
}

async function finalizeLoopRun({
  outputDir,
  repoId,
  repoRootPath,
  manifest,
  paths,
  previousArtifacts,
  manifests,
  mergedTelemetry,
  batchResults,
  runs,
  selectedReviewVerdictsPath,
  generatedArtifacts,
}) {
  const { reviewPacket, selectedReviewVerdicts, scorecard, backlog } = generatedArtifacts;
  const stableArtifacts = buildSummaryArtifacts({
    outputDir,
    stableReviewPacketJsonPath: paths.stableReviewPacketJsonPath,
    reviewPacketJsonPath: paths.reviewPacketJsonPath,
    reviewPacket,
    previousReviewPacketSnapshotPath:
      previousArtifacts.previousReviewPacketSnapshotPath,
    selectedReviewVerdictsPath,
    stableReviewVerdictsOutputPath: paths.stableReviewVerdictsOutputPath,
    runReviewVerdictsOutputPath: paths.runReviewVerdictsOutputPath,
    stableScorecardJsonPath: paths.stableScorecardJsonPath,
    scorecardJsonPath: paths.scorecardJsonPath,
    previousScorecardSnapshotPath: previousArtifacts.previousScorecardSnapshotPath,
    stableBacklogJsonPath: paths.stableBacklogJsonPath,
    backlogJsonPath: paths.backlogJsonPath,
    previousBacklogSnapshotPath: previousArtifacts.previousBacklogSnapshotPath,
    mergedTelemetryJsonPath: paths.mergedTelemetryJsonPath,
    codexBatchResult: batchResults.codexBatchResult,
    codexBatchOutputDir: paths.codexBatchOutputDir,
    replayBatchResult: batchResults.replayBatchResult,
    replayBatchOutputDir: paths.replayBatchOutputDir,
    selectedReviewVerdicts,
    scorecard,
    backlog,
  });
  const warnings = await buildLoopWarningSet({
    paths,
    reviewPacket,
    selectedReviewVerdicts,
    selectedReviewVerdictsPath,
    manifests,
    batchResults,
  });
  const summary = buildLoopSummary({
    outputDir,
    repoId,
    repoRootPath,
    manifest,
    mergedTelemetry,
    reviewPacket,
    selectedReviewVerdicts,
    scorecard,
    backlog,
    previousArtifacts,
    stableArtifacts,
    batchResults,
    warnings,
    runs,
  });

  await publishStableLoopArtifacts(paths, selectedReviewVerdictsPath);
  await writeLoopOutputs(outputDir, summary);
  return summary;
}

async function main() {
  const args = parseArgs(process.argv);
  const manifestPath = path.resolve(args.manifestPath);
  const manifest = await loadRepoCalibrationManifest(manifestPath);
  const repoRootPath = resolveManifestPath(manifestPath, manifest.repo_root);
  const outputDir = path.resolve(
    args.outputDir ??
      defaultBatchOutputDir(
        repoRootPath,
        'repo-calibration-loop',
        manifest.repo_id ?? path.basename(repoRootPath),
      ),
  );
  const repoId = manifest.repo_id ?? path.basename(repoRootPath);
  const lockPath = path.join(
    repoRootPath,
    '.sentrux',
    'evals',
    `.repo-calibration-${repoId}.lock`,
  );
  const releaseLoopLock = await acquireLoopLock(lockPath, {
    repo_id: repoId,
    repo_root: repoRootPath,
    output_dir: outputDir,
    pid: process.pid,
    started_at: nowIso(),
    manifest_path: manifestPath,
  });

  try {
    const paths = resolveLoopArtifactPaths({ manifest, manifestPath, repoRootPath, outputDir });
    const manifests = await loadLoopBatchManifests(paths);
    const previousArtifacts = await capturePreviousArtifacts(outputDir, paths);
    const {
      runs,
      batchResults,
      mergedTelemetry,
      selectedReviewVerdictsPath,
      generatedArtifacts,
    } = await runLoopStages({
      args,
      manifest,
      paths,
      repoRootPath,
    });
    const summary = await finalizeLoopRun({
      outputDir,
      repoId,
      repoRootPath,
      manifest,
      paths,
      previousArtifacts,
      manifests,
      mergedTelemetry,
      batchResults,
      runs,
      selectedReviewVerdictsPath,
      generatedArtifacts,
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
