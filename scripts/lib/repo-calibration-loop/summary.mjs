import {
  buildBatchExpectationWarnings,
  buildBatchFailureWarnings,
  buildSummaryDelta,
  buildWarnings,
} from '../repo-calibration-loop-support.mjs';

export async function buildLoopWarningSet({
  paths,
  reviewPacket,
  selectedReviewVerdicts,
  selectedReviewVerdictsPath,
  manifests,
  batchResults,
  existingPathOrNull,
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

export function buildLoopSummary({
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
  nowIso,
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
