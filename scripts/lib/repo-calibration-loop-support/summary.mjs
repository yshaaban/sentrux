import path from 'node:path';

import { countReviewSamples } from './review-scorecard.mjs';

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
    (previousScorecard.signals ?? []).map(function mapPreviousSignal(signal) {
      return [signal.signal_kind, signal.promotion_recommendation];
    }),
  );

  return (currentScorecard.signals ?? [])
    .map(function mapCurrentSignal(signal) {
      return {
        signal_kind: signal.signal_kind,
        previous: previousBySignalKind.get(signal.signal_kind) ?? null,
        current: signal.promotion_recommendation ?? null,
      };
    })
    .filter(function changedRecommendation(entry) {
      return entry.previous !== null && entry.previous !== entry.current;
    });
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

function formatArtifactValue(value, fallbackLabel) {
  if (!value) {
    return fallbackLabel;
  }

  return `\`${value}\``;
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
  lines.push(
    `- live batch: ${formatArtifactValue(summary.artifacts.codex_batch_json, 'skipped')}`,
  );
  lines.push(
    `- replay batch: ${formatArtifactValue(summary.artifacts.replay_batch_json, 'skipped')}`,
  );
  lines.push(
    `- merged telemetry: ${formatArtifactValue(summary.artifacts.session_telemetry_json, 'none')}`,
  );
  lines.push(
    `- review packet: ${formatArtifactValue(summary.artifacts.review_packet_json, 'skipped')}`,
  );
  lines.push(
    `- session verdicts: ${formatArtifactValue(summary.artifacts.session_verdicts_output, 'missing')}`,
  );
  lines.push(
    `- scorecard: ${formatArtifactValue(summary.artifacts.scorecard_json, 'skipped')}`,
  );
  lines.push(
    `- session corpus: ${formatArtifactValue(summary.artifacts.session_corpus_json, 'skipped')}`,
  );
  lines.push(
    `- backlog: ${formatArtifactValue(summary.artifacts.backlog_json, 'skipped')}`,
  );
  lines.push(
    `- evidence review: ${formatArtifactValue(summary.artifacts.evidence_review_json, 'skipped')}`,
  );
  lines.push('');
  lines.push('## Summary');
  lines.push('');
  lines.push(`- total sessions: ${summary.summary.session_count}`);
  lines.push(`- corpus sessions: ${summary.summary.corpus_session_count ?? 0}`);
  lines.push(`- total signals: ${summary.summary.total_signals ?? 0}`);
  lines.push(`- weak signals: ${summary.summary.weak_signal_count ?? 0}`);
  lines.push(`- review samples: ${summary.summary.review_sample_count ?? 0}`);
  lines.push(`- session verdicts: ${summary.summary.session_verdict_count ?? 0}`);
  lines.push(`- live clean rate: ${summary.summary.live_clean_rate ?? 'n/a'}`);
  lines.push(`- replay clean rate: ${summary.summary.replay_clean_rate ?? 'n/a'}`);
  lines.push(`- corpus agent clear rate: ${summary.summary.agent_clear_rate ?? 'n/a'}`);
  lines.push(`- top-action follow rate: ${summary.summary.top_action_follow_rate ?? 'n/a'}`);
  lines.push(`- top-action help rate: ${summary.summary.top_action_help_rate ?? 'n/a'}`);
  lines.push(`- task success rate: ${summary.summary.task_success_rate ?? 'n/a'}`);
  lines.push(`- patch expansion rate: ${summary.summary.patch_expansion_rate ?? 'n/a'}`);
  lines.push(
    `- reviewer acceptance rate: ${summary.summary.reviewer_acceptance_rate ?? 'n/a'}`,
  );
  lines.push(
    `- reviewer disagreement rate: ${summary.summary.reviewer_disagreement_rate ?? 'n/a'}`,
  );
  lines.push(`- intervention net value score: ${summary.summary.intervention_net_value_score ?? 'n/a'}`);
  lines.push(`- propagation escape rate: ${summary.summary.propagation_escape_rate ?? 'n/a'}`);
  lines.push(`- clone followthrough escape rate: ${summary.summary.clone_followthrough_escape_rate ?? 'n/a'}`);
  lines.push(
    `- signal-matched ready signals: ${summary.summary.signal_matched_ready_count ?? 0}`,
  );
  lines.push(`- default-on candidates: ${summary.summary.evidence_review_default_on_candidates ?? 0}`);
  lines.push(
    `- default-on ready signals: ${summary.summary.evidence_review_default_on_ready_signals ?? 0}`,
  );
  lines.push(`- default-on ready: ${summary.summary.default_on_ready ? 'true' : 'false'}`);
  lines.push(
    `- repo treatment ready: ${summary.summary.default_on_repo_treatment_ready ? 'true' : 'false'}`,
  );
  lines.push(`- default-on evidence scope: ${summary.summary.default_on_evidence_scope ?? 'n/a'}`);
  lines.push(
    `- signal treatment ready count: ${summary.summary.default_on_signal_treatment_ready_count ?? 0}`,
  );
  lines.push(`- evidence phase: ${summary.summary.evidence_phase_id ?? 'n/a'}`);
  lines.push(
    `- bounded adjudication status: ${summary.summary.bounded_adjudication_status ?? 'n/a'}`,
  );
  lines.push(
    `- bounded adjudication decisions: ${summary.summary.bounded_adjudication_decision_count ?? 0}`,
  );
  lines.push(
    `- bounded adjudication structured-only: ${
      summary.summary.bounded_adjudication_structured_evidence_only === null
        ? 'n/a'
        : summary.summary.bounded_adjudication_structured_evidence_only
          ? 'true'
          : 'false'
    }`,
  );
  lines.push(
    `- bounded adjudication audit logging ready: ${
      summary.summary.bounded_adjudication_audit_logging_ready === null
        ? 'n/a'
        : summary.summary.bounded_adjudication_audit_logging_ready
          ? 'true'
          : 'false'
    }`,
  );
  lines.push(
    `- bounded adjudication auto-apply enabled: ${
      summary.summary.bounded_adjudication_auto_apply_enabled === null
        ? 'n/a'
        : summary.summary.bounded_adjudication_auto_apply_enabled
          ? 'true'
          : 'false'
    }`,
  );
  lines.push(
    `- bounded adjudication phase: ${summary.summary.bounded_adjudication_phase_id ?? 'n/a'}`,
  );
  lines.push(`- next signal: ${summary.summary.recommended_next_signal ?? 'none'}`);
  lines.push('');

  if (summary.delta) {
    lines.push('## Delta');
    lines.push('');
    lines.push(`- total signals delta: ${summary.delta.total_signals?.delta ?? 'n/a'}`);
    lines.push(`- weak signals delta: ${summary.delta.weak_signal_count?.delta ?? 'n/a'}`);
    lines.push(`- review samples delta: ${summary.delta.review_sample_count?.delta ?? 'n/a'}`);
    lines.push(`- session verdicts delta: ${summary.delta.session_verdict_count?.delta ?? 'n/a'}`);
    lines.push(`- live clean rate delta: ${summary.delta.live_clean_rate?.delta ?? 'n/a'}`);
    lines.push(`- replay clean rate delta: ${summary.delta.replay_clean_rate?.delta ?? 'n/a'}`);
    lines.push(`- corpus agent clear rate delta: ${summary.delta.agent_clear_rate?.delta ?? 'n/a'}`);
    lines.push(`- top-action help rate delta: ${summary.delta.top_action_help_rate?.delta ?? 'n/a'}`);
    lines.push(`- task success rate delta: ${summary.delta.task_success_rate?.delta ?? 'n/a'}`);
    lines.push(
      `- reviewer acceptance rate delta: ${summary.delta.reviewer_acceptance_rate?.delta ?? 'n/a'}`,
    );
    lines.push(
      `- reviewer disagreement rate delta: ${summary.delta.reviewer_disagreement_rate?.delta ?? 'n/a'}`,
    );
    lines.push(
      `- signal-matched ready signals delta: ${summary.delta.signal_matched_ready_count?.delta ?? 'n/a'}`,
    );
    lines.push(
      `- default-on ready signals delta: ${summary.delta.default_on_ready_signal_count?.delta ?? 'n/a'}`,
    );
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

export function buildSummaryDelta(
  currentScorecard,
  currentSessionCorpus,
  previousScorecard,
  previousSessionCorpus,
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
    session_verdict_count: buildNumericDelta(
      currentSessionCorpus?.summary?.session_verdict_count ?? 0,
      previousSessionCorpus?.summary?.session_verdict_count ?? 0,
    ),
    live_clean_rate: buildNumericDelta(
      currentBacklog?.summary?.live_clean_rate,
      previousBacklog?.summary?.live_clean_rate,
    ),
    replay_clean_rate: buildNumericDelta(
      currentBacklog?.summary?.replay_clean_rate,
      previousBacklog?.summary?.replay_clean_rate,
    ),
    agent_clear_rate: buildNumericDelta(
      currentSessionCorpus?.summary?.agent_clear_rate,
      previousSessionCorpus?.summary?.agent_clear_rate,
    ),
    top_action_help_rate: buildNumericDelta(
      currentSessionCorpus?.summary?.top_action_help_rate,
      previousSessionCorpus?.summary?.top_action_help_rate,
    ),
    task_success_rate: buildNumericDelta(
      currentSessionCorpus?.summary?.task_success_rate,
      previousSessionCorpus?.summary?.task_success_rate,
    ),
    reviewer_acceptance_rate: buildNumericDelta(
      currentSessionCorpus?.summary?.reviewer_acceptance_rate,
      previousSessionCorpus?.summary?.reviewer_acceptance_rate,
    ),
    reviewer_disagreement_rate: buildNumericDelta(
      currentSessionCorpus?.summary?.reviewer_disagreement_rate,
      previousSessionCorpus?.summary?.reviewer_disagreement_rate,
    ),
    signal_matched_ready_count: buildNumericDelta(
      currentSessionCorpus?.summary?.signal_experiment_ready_count ?? 0,
      previousSessionCorpus?.summary?.signal_experiment_ready_count ?? 0,
    ),
    default_on_ready_signal_count: buildNumericDelta(
      currentScorecard?.summary?.default_rollout_ready_count ?? 0,
      previousScorecard?.summary?.default_rollout_ready_count ?? 0,
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
  selectedSessionVerdictsPath,
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
  if (!selectedSessionVerdictsPath) {
    warnings.push('session verdict input missing; product-value metrics have no curated session-level evidence');
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
  stableReviewPacketJsonPath,
  reviewPacketJsonPath,
  reviewPacket,
  previousReviewPacketSnapshotPath,
  selectedReviewVerdictsPath,
  stableReviewVerdictsOutputPath,
  runReviewVerdictsOutputPath,
  selectedSessionVerdictsPath,
  stableSessionVerdictsOutputPath,
  runSessionVerdictsOutputPath,
  previousSessionVerdictsSnapshotPath,
  stableScorecardJsonPath,
  scorecardJsonPath,
  previousScorecardSnapshotPath,
  stableSessionCorpusJsonPath,
  sessionCorpusJsonPath,
  previousSessionCorpusSnapshotPath,
  stableBacklogJsonPath,
  backlogJsonPath,
  previousBacklogSnapshotPath,
  stableEvidenceReviewJsonPath,
  evidenceReviewJsonPath,
  previousEvidenceReviewSnapshotPath,
  mergedTelemetryJsonPath,
  codexBatchResult,
  codexBatchOutputDir,
  replayBatchResult,
  replayBatchOutputDir,
  selectedReviewVerdicts,
  selectedSessionVerdicts,
  scorecard,
  sessionCorpus,
  backlog,
  evidenceReview,
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
    session_verdicts_input: selectedSessionVerdictsPath,
    session_verdicts_output:
      stableSessionVerdictsOutputPath ?? runSessionVerdictsOutputPath,
    session_verdicts_run_output: selectedSessionVerdicts
      ? runSessionVerdictsOutputPath
      : null,
    previous_session_verdicts_json: previousSessionVerdictsSnapshotPath,
    scorecard_json: stableScorecardJsonPath ?? scorecardJsonPath,
    scorecard_run_json: scorecard ? scorecardJsonPath : null,
    previous_scorecard_json: previousScorecardSnapshotPath,
    session_corpus_json: stableSessionCorpusJsonPath ?? sessionCorpusJsonPath,
    session_corpus_run_json: sessionCorpus ? sessionCorpusJsonPath : null,
    previous_session_corpus_json: previousSessionCorpusSnapshotPath,
    backlog_json: stableBacklogJsonPath ?? backlogJsonPath,
    backlog_run_json: backlog ? backlogJsonPath : null,
    previous_backlog_json: previousBacklogSnapshotPath,
    evidence_review_json: stableEvidenceReviewJsonPath ?? evidenceReviewJsonPath,
    evidence_review_run_json: evidenceReview ? evidenceReviewJsonPath : null,
    previous_evidence_review_json: previousEvidenceReviewSnapshotPath,
  };
}
