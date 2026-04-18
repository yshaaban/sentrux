import path from 'node:path';
import {
  buildBatchRunArgs,
  buildReviewArgs,
  buildScorecardArgs,
  maybeBuildProvisionalReviewVerdicts,
  runNodeScript,
  selectReviewVerdictsPath,
  selectSessionVerdictsPath,
} from '../repo-calibration-loop-support.mjs';
import {
  formatSessionTelemetrySummaryMarkdown,
  mergeSessionTelemetrySummaries,
} from '../session-telemetry.mjs';
import { pathExists } from '../repo-calibration-artifacts.mjs';
import { readJson, writeJson, writeText } from '../eval-batch.mjs';

async function runBatchLane({
  skip,
  manifestPath,
  outputDir,
  cohortManifestPath,
  cohortId,
  scriptName,
  resultPath,
  runs,
  repoRoot,
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
    sourcePaths: telemetrySummaries.flatMap(function collectPaths(summary) {
      return summary.source_paths ?? [];
    }),
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
  repoRoot,
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
  reviewPacketJsonPath,
  codexBatchResult,
  codexBatchResultPath,
  replayBatchResult,
  replayBatchResultPath,
  defectReportPath,
  selectedReviewVerdictsPath,
  selectedSessionVerdictsPath,
  remediationReportPath,
  benchmarkPath,
  runs,
  repoRoot,
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
    reviewPacketJsonPath,
    codexBatchPath: codexBatchResult ? codexBatchResultPath : null,
    replayBatchPath: replayBatchResult ? replayBatchResultPath : null,
    defectReportPath,
    selectedReviewVerdictsPath,
    selectedSessionVerdictsPath,
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

async function runSessionCorpusStage({
  mergedTelemetryJsonPath,
  sessionCorpusJsonPath,
  sessionCorpusMarkdownPath,
  selectedSessionVerdictsPath,
  codexBatchResult,
  codexBatchResultPath,
  replayBatchResult,
  replayBatchResultPath,
  manifest,
  repoRootPath,
  runs,
  repoRoot,
}) {
  const sessionCorpusArgs = [
    '--repo-root',
    repoRootPath,
    '--repo-label',
    manifest.repo_label ?? manifest.repo_id ?? path.basename(repoRootPath),
    '--session-telemetry',
    mergedTelemetryJsonPath,
    '--output-json',
    sessionCorpusJsonPath,
    '--output-md',
    sessionCorpusMarkdownPath,
  ];

  if (codexBatchResult) {
    sessionCorpusArgs.push('--codex-batch', codexBatchResultPath);
  }
  if (replayBatchResult) {
    sessionCorpusArgs.push('--replay-batch', replayBatchResultPath);
  }
  if (selectedSessionVerdictsPath) {
    sessionCorpusArgs.push('--session-verdicts', selectedSessionVerdictsPath);
  }

  runs.push(
    await runNodeScript(
      repoRoot,
      path.join(repoRoot, 'scripts/evals/build-session-corpus.mjs'),
      sessionCorpusArgs,
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
  repoRoot,
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

async function runEvidenceReviewStage({
  skipBacklog,
  scorecardJsonPath,
  backlogJsonPath,
  sessionCorpusJsonPath,
  reviewPacketJsonPath,
  evidenceReviewJsonPath,
  evidenceReviewMarkdownPath,
  runs,
  repoRoot,
}) {
  if (skipBacklog) {
    return;
  }
  if (
    !(await pathExists(scorecardJsonPath)) ||
    !(await pathExists(backlogJsonPath)) ||
    !(await pathExists(sessionCorpusJsonPath))
  ) {
    throw new Error(
      'Cannot build evidence review without scorecard, backlog, and session corpus artifacts',
    );
  }

  const evidenceReviewArgs = [
    '--scorecard',
    scorecardJsonPath,
    '--backlog',
    backlogJsonPath,
    '--session-corpus',
    sessionCorpusJsonPath,
    '--output-json',
    evidenceReviewJsonPath,
    '--output-md',
    evidenceReviewMarkdownPath,
  ];
  if (await pathExists(reviewPacketJsonPath)) {
    evidenceReviewArgs.push('--review-packet', reviewPacketJsonPath);
  }

  runs.push(
    await runNodeScript(
      repoRoot,
      path.join(repoRoot, 'scripts/evals/build-evidence-review.mjs'),
      evidenceReviewArgs,
    ),
  );
}

async function maybeRunProvisionalVerdictStage(args, manifest, paths, runs, repoRoot) {
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

async function resolveSelectedSessionVerdictsPath(paths) {
  return selectSessionVerdictsPath(
    paths.stableSessionVerdictsOutputPath,
    paths.sessionVerdictsPath,
  );
}

export async function runLoopStages({ args, manifest, paths, repoRootPath, repoRoot }) {
  const runs = [];
  const batchResults = await runBatchStages({
    args,
    manifest,
    paths,
    repoRootPath,
    runs,
    repoRoot,
  });
  const selectedPaths = await runAnalysisStages({
    args,
    manifest,
    paths,
    repoRootPath,
    batchResults,
    runs,
    repoRoot,
  });

  return {
    runs,
    batchResults,
    mergedTelemetry: batchResults.mergedTelemetry,
    selectedReviewVerdictsPath: selectedPaths.selectedReviewVerdictsPath,
    selectedSessionVerdictsPath: selectedPaths.selectedSessionVerdictsPath,
  };
}

async function runBatchStages({ args, manifest, paths, repoRootPath, runs, repoRoot }) {
  const codexBatchResult = await runBatchLane({
    skip: args.skipLive,
    manifestPath: paths.codexBatchManifestPath,
    outputDir: paths.codexBatchOutputDir,
    cohortManifestPath: paths.cohortManifestPath,
    cohortId: manifest.cohort_id,
    scriptName: 'scripts/evals/run-codex-session-batch.mjs',
    resultPath: paths.codexBatchResultPath,
    runs,
    repoRoot,
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
    repoRoot,
  });
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

  return {
    codexBatchResult,
    replayBatchResult,
    mergedTelemetry,
  };
}

async function runAnalysisStages({
  args,
  manifest,
  paths,
  repoRootPath,
  batchResults,
  runs,
  repoRoot,
}) {
  await runReviewStage({
    skipReview: args.skipReview,
    manifest,
    reviewPacketJsonPath: paths.reviewPacketJsonPath,
    reviewPacketMarkdownPath: paths.reviewPacketMarkdownPath,
    codexBatchResult: batchResults.codexBatchResult,
    codexBatchResultPath: paths.codexBatchResultPath,
    replayBatchResult: batchResults.replayBatchResult,
    replayBatchResultPath: paths.replayBatchResultPath,
    runs,
    repoRoot,
  });
  await maybeRunProvisionalVerdictStage(args, manifest, paths, runs, repoRoot);

  const selectedReviewVerdictsPath = await resolveSelectedReviewVerdictsPath(args, paths);
  const selectedSessionVerdictsPath = await resolveSelectedSessionVerdictsPath(paths);

  await runScorecardStage({
    skipScorecard: args.skipScorecard,
    manifest,
    repoRootPath,
    mergedTelemetryJsonPath: paths.mergedTelemetryJsonPath,
    scorecardJsonPath: paths.scorecardJsonPath,
    scorecardMarkdownPath: paths.scorecardMarkdownPath,
    reviewPacketJsonPath: paths.reviewPacketJsonPath,
    codexBatchResult: batchResults.codexBatchResult,
    codexBatchResultPath: paths.codexBatchResultPath,
    replayBatchResult: batchResults.replayBatchResult,
    replayBatchResultPath: paths.replayBatchResultPath,
    defectReportPath: paths.defectReportPath,
    selectedReviewVerdictsPath,
    selectedSessionVerdictsPath,
    remediationReportPath: paths.remediationReportPath,
    benchmarkPath: paths.benchmarkPath,
    runs,
    repoRoot,
  });
  await runPostScoreStages({
    skipBacklog: args.skipBacklog,
    manifest,
    paths,
    repoRootPath,
    batchResults,
    selectedSessionVerdictsPath,
    runs,
    repoRoot,
  });

  return {
    selectedReviewVerdictsPath,
    selectedSessionVerdictsPath,
  };
}

async function runPostScoreStages({
  skipBacklog,
  manifest,
  paths,
  repoRootPath,
  batchResults,
  selectedSessionVerdictsPath,
  runs,
  repoRoot,
}) {
  await runSessionCorpusStage({
    mergedTelemetryJsonPath: paths.mergedTelemetryJsonPath,
    sessionCorpusJsonPath: paths.sessionCorpusJsonPath,
    sessionCorpusMarkdownPath: paths.sessionCorpusMarkdownPath,
    selectedSessionVerdictsPath,
    codexBatchResult: batchResults.codexBatchResult,
    codexBatchResultPath: paths.codexBatchResultPath,
    replayBatchResult: batchResults.replayBatchResult,
    replayBatchResultPath: paths.replayBatchResultPath,
    manifest,
    repoRootPath,
    runs,
    repoRoot,
  });
  await runBacklogStage({
    skipBacklog,
    scorecardJsonPath: paths.scorecardJsonPath,
    backlogJsonPath: paths.backlogJsonPath,
    backlogMarkdownPath: paths.backlogMarkdownPath,
    cohortManifestPath: paths.cohortManifestPath,
    cohortId: manifest.cohort_id,
    codexBatchResult: batchResults.codexBatchResult,
    codexBatchResultPath: paths.codexBatchResultPath,
    replayBatchResult: batchResults.replayBatchResult,
    replayBatchResultPath: paths.replayBatchResultPath,
    runs,
    repoRoot,
  });
  await runEvidenceReviewStage({
    skipBacklog,
    scorecardJsonPath: paths.scorecardJsonPath,
    backlogJsonPath: paths.backlogJsonPath,
    sessionCorpusJsonPath: paths.sessionCorpusJsonPath,
    reviewPacketJsonPath: paths.reviewPacketJsonPath,
    evidenceReviewJsonPath: paths.evidenceReviewJsonPath,
    evidenceReviewMarkdownPath: paths.evidenceReviewMarkdownPath,
    runs,
    repoRoot,
  });
}
