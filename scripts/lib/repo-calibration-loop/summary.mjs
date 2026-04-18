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
  selectedSessionVerdictsPath,
  manifests,
  batchResults,
  existingPathOrNull,
}) {
  return [
    ...buildWarnings(
      selectedReviewVerdictsPath,
      selectedSessionVerdictsPath,
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
  selectedSessionVerdicts,
  scorecard,
  sessionCorpus,
  backlog,
  evidenceReview,
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
      corpus_session_count: sessionCorpus?.summary?.session_count ?? 0,
      total_signals: scorecard?.summary?.total_signals ?? 0,
      weak_signal_count: backlog?.summary?.weak_signal_count ?? 0,
      review_sample_count:
        reviewPacket?.summary?.sample_count ?? reviewPacket?.samples?.length ?? 0,
      session_verdict_count: selectedSessionVerdicts?.verdicts?.length ?? 0,
      live_clean_rate: backlog?.summary?.live_clean_rate ?? null,
      replay_clean_rate: backlog?.summary?.replay_clean_rate ?? null,
      agent_clear_rate: sessionCorpus?.summary?.agent_clear_rate ?? null,
      top_action_follow_rate: sessionCorpus?.summary?.top_action_follow_rate ?? null,
      top_action_help_rate: sessionCorpus?.summary?.top_action_help_rate ?? null,
      task_success_rate: sessionCorpus?.summary?.task_success_rate ?? null,
      patch_expansion_rate: sessionCorpus?.summary?.patch_expansion_rate ?? null,
      reviewer_acceptance_rate: sessionCorpus?.summary?.reviewer_acceptance_rate ?? null,
      reviewer_disagreement_rate: sessionCorpus?.summary?.reviewer_disagreement_rate ?? null,
      intervention_net_value_score:
        sessionCorpus?.summary?.intervention_net_value_score ?? null,
      propagation_escape_rate: sessionCorpus?.summary?.propagation_escape_rate ?? null,
      clone_followthrough_escape_rate:
        sessionCorpus?.summary?.clone_followthrough_escape_rate ?? null,
      signal_matched_ready_count:
        sessionCorpus?.summary?.signal_experiment_ready_count ?? 0,
      recommended_next_signal: backlog?.summary?.recommended_next_signal ?? null,
      live_failure_count: batchResults.codexBatchResult?.failure_count ?? 0,
      replay_failure_count: batchResults.replayBatchResult?.failure_count ?? 0,
      evidence_review_promotion_candidates:
        evidenceReview?.summary?.promotion_candidate_count ?? 0,
      evidence_review_default_on_candidates:
        evidenceReview?.summary?.default_on_candidate_count ?? 0,
      evidence_review_default_on_ready_signals:
        evidenceReview?.summary?.default_on_ready_signal_count ?? 0,
      default_on_ready: evidenceReview?.default_on_promotion?.ready ?? false,
      default_on_repo_treatment_ready:
        evidenceReview?.default_on_promotion?.repo_treatment_ready ?? false,
      default_on_evidence_scope:
        evidenceReview?.default_on_promotion?.evidence_scope ?? null,
      default_on_signal_treatment_ready_count:
        evidenceReview?.default_on_promotion?.signal_treatment_ready_count ?? 0,
      evidence_phase_id:
        evidenceReview?.evidence_sources?.live?.phase_id ??
        evidenceReview?.evidence_sources?.replay?.phase_id ??
        null,
      bounded_adjudication_status:
        evidenceReview?.adjudication_summary?.status ??
        sessionCorpus?.adjudication_summary?.status ??
        null,
      bounded_adjudication_decision_count:
        evidenceReview?.adjudication_summary?.decision_count ??
        sessionCorpus?.adjudication_summary?.decision_count ??
        0,
      bounded_adjudication_structured_evidence_only:
        evidenceReview?.adjudication_summary?.structured_evidence_only ??
        sessionCorpus?.adjudication_summary?.structured_evidence_only ??
        null,
      bounded_adjudication_audit_logging_ready:
        evidenceReview?.adjudication_summary?.audit_logging_ready ??
        sessionCorpus?.adjudication_summary?.audit_logging_ready ??
        null,
      bounded_adjudication_auto_apply_enabled:
        evidenceReview?.adjudication_summary?.auto_apply_enabled ??
        sessionCorpus?.adjudication_summary?.auto_apply_enabled ??
        null,
      bounded_adjudication_phase_id:
        evidenceReview?.phase_tracking?.bounded_llm_adjudication?.phase_id ??
        sessionCorpus?.phase_tracking?.bounded_llm_adjudication?.phase_id ??
        null,
    },
    delta: buildSummaryDelta(
      scorecard,
      sessionCorpus,
      previousArtifacts.previousScorecard,
      previousArtifacts.previousSessionCorpus,
      backlog,
      previousArtifacts.previousBacklog,
      reviewPacket,
      previousArtifacts.previousReviewPacket,
    ),
    warnings,
    runs,
  };
}
