import { SIGNAL_BACKLOG_PRIORITY_WEIGHTS } from './signal-calibration-policy.mjs';
import { asArray, ensureMapEntry, safeRatio } from './signal-summary-utils.mjs';

function isKeepRecommendation(value) {
  return typeof value === 'string' && value.startsWith('keep_');
}

function summarizeWeakSignals(cohort, scorecard, candidateMap) {
  const activeSignalKinds = new Set(cohort.signals.map((signal) => signal.signal_kind));

  return asArray(scorecard?.signals)
    .filter((signal) => activeSignalKinds.has(signal.signal_kind))
    .filter((signal) => !isKeepRecommendation(signal.promotion_recommendation))
    .map((signal) => {
      const candidate = candidateMap.get(signal.signal_kind);

      return {
        signal_kind: signal.signal_kind,
        promotion_status: signal.promotion_status,
        recommendation: signal.promotion_recommendation,
        seeded_recall: signal.seeded_recall ?? null,
        reviewed_precision: signal.reviewed_precision ?? null,
        remediation_success_rate: signal.remediation_success_rate ?? null,
        session_resolution_rate: signal.session_resolution_rate ?? null,
        session_clean_rate: signal.session_clean_rate ?? null,
        session_trial_count: signal.session_trial_count ?? 0,
        session_trial_miss_rate: signal.session_trial_miss_rate ?? null,
        expected_missing_count: candidate?.expected_missing_count ?? 0,
        expected_present_not_top_count: candidate?.expected_present_not_top_count ?? 0,
        crowded_out_expected_count: candidate?.crowded_out_expected_count ?? 0,
        unexpected_top_action_count: candidate?.unexpected_top_action_count ?? 0,
        telemetry_incomplete_count: candidate?.telemetry_incomplete_count ?? 0,
        average_checks_to_clear: signal.average_checks_to_clear ?? null,
      };
    });
}

function createCandidateEntry(signalKind) {
  return {
    signal_kind: signalKind,
    miss_count: 0,
    replay_miss_count: 0,
    live_miss_count: 0,
    regression_followup_count: 0,
    expected_missing_count: 0,
    expected_present_not_top_count: 0,
    crowded_out_expected_count: 0,
    unexpected_top_action_count: 0,
    telemetry_incomplete_count: 0,
    not_in_active_cohort: 0,
    priority_score: 0,
  };
}

function ensureCandidateEntry(candidateMap, signalKind) {
  return ensureMapEntry(candidateMap, signalKind, createCandidateEntry);
}

function recordCandidate(candidateMap, signalKind, bucket, activeSignalKinds) {
  if (!signalKind) {
    return null;
  }

  const entry = ensureCandidateEntry(candidateMap, signalKind);
  entry.miss_count += 1;
  entry[`${bucket}_miss_count`] += 1;
  if (!activeSignalKinds.has(signalKind)) {
    entry.not_in_active_cohort += 1;
  }
  return entry;
}

function buildMissEntries(results, lane, activeSignalKinds, candidateMap) {
  const misses = [];

  for (const result of results) {
    const outcome = result.outcome ?? {};
    const expectedKinds = asArray(result.expected_signal_kinds);
    const initialTopActionKind = outcome.initial_top_action_kind ?? null;
    const initialActionKinds = new Set(asArray(outcome.initial_action_kinds));
    const telemetryIncomplete = Boolean(initialTopActionKind) && initialActionKinds.size === 0;
    const presentExpectedKinds = telemetryIncomplete
      ? []
      : expectedKinds.filter((kind) => initialActionKinds.has(kind));
    const missingExpectedKinds = telemetryIncomplete
      ? []
      : expectedKinds.filter((kind) => !initialActionKinds.has(kind));
    const expectedKindsPresentButNotTop =
      initialTopActionKind && !expectedKinds.includes(initialTopActionKind)
        ? presentExpectedKinds
        : [];
    const needsAttention =
      !outcome.final_session_clean ||
      outcome.followup_regression_introduced ||
      ((missingExpectedKinds.length > 0 || expectedKindsPresentButNotTop.length > 0) &&
        outcome.final_session_clean === false);

    if (!needsAttention) {
      continue;
    }

    if (telemetryIncomplete) {
      ensureCandidateEntry(candidateMap, initialTopActionKind).telemetry_incomplete_count += 1;
    } else {
      for (const signalKind of missingExpectedKinds) {
        const candidate = recordCandidate(candidateMap, signalKind, lane, activeSignalKinds);
        candidate.expected_missing_count += 1;
      }

      for (const signalKind of expectedKindsPresentButNotTop) {
        const candidate = recordCandidate(candidateMap, signalKind, lane, activeSignalKinds);
        candidate.expected_present_not_top_count += 1;
      }
    }

    if (
      !telemetryIncomplete &&
      initialTopActionKind &&
      !expectedKinds.includes(initialTopActionKind) &&
      expectedKindsPresentButNotTop.length > 0
    ) {
      ensureCandidateEntry(candidateMap, initialTopActionKind).crowded_out_expected_count += 1;
    } else if (
      !telemetryIncomplete &&
      missingExpectedKinds.length > 0 &&
      initialTopActionKind &&
      !expectedKinds.includes(initialTopActionKind)
    ) {
      ensureCandidateEntry(candidateMap, initialTopActionKind).unexpected_top_action_count += 1;
    }

    if (outcome.followup_regression_introduced && initialTopActionKind) {
      ensureCandidateEntry(candidateMap, initialTopActionKind).regression_followup_count += 1;
    }

    misses.push({
      id: result.task_id ?? result.replay_id ?? null,
      lane,
      label: result.task_label ?? result.commit ?? null,
      expected_signal_kinds: expectedKinds,
      initial_action_kinds: [...initialActionKinds],
      telemetry_incomplete: telemetryIncomplete,
      missing_expected_kinds: missingExpectedKinds,
      expected_present_not_top_kinds: expectedKindsPresentButNotTop,
      initial_top_action_kind: initialTopActionKind,
      top_action_cleared: outcome.top_action_cleared ?? false,
      final_gate: outcome.final_gate ?? null,
      final_session_clean: outcome.final_session_clean ?? false,
      followup_regression_introduced: outcome.followup_regression_introduced ?? false,
      output_dir: result.output_dir ?? null,
    });
  }

  return misses;
}

function summarizeNextCandidates(candidateMap) {
  return [...candidateMap.values()]
    .map((candidate) => ({
      ...candidate,
      priority_score:
        candidate.live_miss_count * SIGNAL_BACKLOG_PRIORITY_WEIGHTS.liveMiss +
        candidate.replay_miss_count * SIGNAL_BACKLOG_PRIORITY_WEIGHTS.replayMiss +
        candidate.regression_followup_count *
          SIGNAL_BACKLOG_PRIORITY_WEIGHTS.regressionFollowup +
        (candidate.not_in_active_cohort > 0
          ? SIGNAL_BACKLOG_PRIORITY_WEIGHTS.outOfCohortBonus
          : 0),
    }))
    .sort((left, right) => {
      if (right.priority_score !== left.priority_score) {
        return right.priority_score - left.priority_score;
      }
      if (right.expected_present_not_top_count !== left.expected_present_not_top_count) {
        return right.expected_present_not_top_count - left.expected_present_not_top_count;
      }
      if (right.expected_missing_count !== left.expected_missing_count) {
        return right.expected_missing_count - left.expected_missing_count;
      }
      if (right.telemetry_incomplete_count !== left.telemetry_incomplete_count) {
        return right.telemetry_incomplete_count - left.telemetry_incomplete_count;
      }
      if (right.crowded_out_expected_count !== left.crowded_out_expected_count) {
        return right.crowded_out_expected_count - left.crowded_out_expected_count;
      }
      if (right.unexpected_top_action_count !== left.unexpected_top_action_count) {
        return right.unexpected_top_action_count - left.unexpected_top_action_count;
      }
      if (right.not_in_active_cohort !== left.not_in_active_cohort) {
        return right.not_in_active_cohort - left.not_in_active_cohort;
      }
      if (right.miss_count !== left.miss_count) {
        return right.miss_count - left.miss_count;
      }
      return left.signal_kind.localeCompare(right.signal_kind);
    });
}

function summarizeConfiguredNextCandidates(cohort, candidateSignals, activeSignalKinds) {
  const configuredCandidates = asArray(cohort?.next_candidates);
  const configuredRanks = new Map(
    configuredCandidates.map((signalKind, index) => [signalKind, index]),
  );
  const candidateBySignal = new Map(
    candidateSignals.map((candidate) => [candidate.signal_kind, candidate]),
  );
  const mergedCandidates = new Map();

  for (const signalKind of configuredCandidates) {
    if (!signalKind || activeSignalKinds.has(signalKind) || mergedCandidates.has(signalKind)) {
      continue;
    }

    mergedCandidates.set(
      signalKind,
      candidateBySignal.get(signalKind) ?? createCandidateEntry(signalKind),
    );
  }

  for (const candidate of candidateSignals) {
    if (candidate.not_in_active_cohort === 0 || mergedCandidates.has(candidate.signal_kind)) {
      continue;
    }

    mergedCandidates.set(candidate.signal_kind, candidate);
  }

  return [...mergedCandidates.values()].sort((left, right) => {
    if (right.priority_score !== left.priority_score) {
      return right.priority_score - left.priority_score;
    }
    if (right.miss_count !== left.miss_count) {
      return right.miss_count - left.miss_count;
    }

    const leftConfiguredRank = configuredRanks.get(left.signal_kind);
    const rightConfiguredRank = configuredRanks.get(right.signal_kind);
    if (leftConfiguredRank != null && rightConfiguredRank != null) {
      return leftConfiguredRank - rightConfiguredRank;
    }
    if (leftConfiguredRank != null) {
      return -1;
    }
    if (rightConfiguredRank != null) {
      return 1;
    }

    return left.signal_kind.localeCompare(right.signal_kind);
  });
}

function cleanRateForResults(results) {
  const normalizedResults = asArray(results);
  return safeRatio(
    normalizedResults.filter((result) => result.outcome?.final_session_clean).length,
    normalizedResults.length,
  );
}

function appendCandidateSection(lines, title, candidates) {
  lines.push(title);
  lines.push('');

  if (candidates.length === 0) {
    lines.push('- none');
  } else {
    for (const candidate of candidates.slice(0, 10)) {
      lines.push(
        `- \`${candidate.signal_kind}\`: score=${candidate.priority_score ?? 0}, misses=${candidate.miss_count}, live=${candidate.live_miss_count}, replay=${candidate.replay_miss_count}, expected_missing=${candidate.expected_missing_count ?? 0}, present_not_top=${candidate.expected_present_not_top_count ?? 0}, regression_followups=${candidate.regression_followup_count}, crowded_out=${candidate.crowded_out_expected_count ?? 0}, unexpected_top=${candidate.unexpected_top_action_count ?? 0}, telemetry_incomplete=${candidate.telemetry_incomplete_count ?? 0}`,
      );
    }
  }

  lines.push('');
}

export function buildSignalBacklog({ cohort, scorecard, codexBatch = null, replayBatch = null }) {
  const activeSignalKinds = new Set(cohort.signals.map((signal) => signal.signal_kind));
  const candidateMap = new Map();
  const liveMisses = buildMissEntries(
    asArray(codexBatch?.results),
    'live',
    activeSignalKinds,
    candidateMap,
  );
  const replayMisses = buildMissEntries(
    asArray(replayBatch?.results),
    'replay',
    activeSignalKinds,
    candidateMap,
  );
  const weakSignals = summarizeWeakSignals(cohort, scorecard, candidateMap);
  const candidateSignals = summarizeNextCandidates(candidateMap);
  const nextCandidates = summarizeConfiguredNextCandidates(
    cohort,
    candidateSignals,
    activeSignalKinds,
  );
  const recommendedNextCandidate =
    nextCandidates.find((candidate) => (candidate.priority_score ?? 0) > 0) ?? null;
  const activeSignalMisses = candidateSignals.filter((candidate) =>
    activeSignalKinds.has(candidate.signal_kind),
  );

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    cohort_id: cohort.cohort_id,
    active_signal_kinds: cohort.signals.map((signal) => signal.signal_kind),
    summary: {
      weak_signal_count: weakSignals.length,
      live_miss_count: liveMisses.length,
      replay_miss_count: replayMisses.length,
      active_signal_miss_count: activeSignalMisses.length,
      next_candidate_count: nextCandidates.length,
      recommended_next_signal: recommendedNextCandidate?.signal_kind ?? null,
      recommended_next_signal_score: recommendedNextCandidate?.priority_score ?? null,
      live_clean_rate: cleanRateForResults(codexBatch?.results),
      replay_clean_rate: cleanRateForResults(replayBatch?.results),
    },
    weak_signals: weakSignals,
    live_misses: liveMisses,
    replay_misses: replayMisses,
    active_signal_misses: activeSignalMisses,
    next_signal_candidates: nextCandidates,
  };
}

export function formatSignalBacklogMarkdown(backlog) {
  const lines = [];
  lines.push('# Signal Calibration Backlog');
  lines.push('');
  lines.push(`- cohort: \`${backlog.cohort_id}\``);
  lines.push(`- generated at: \`${backlog.generated_at}\``);
  lines.push(`- weak signals: ${backlog.summary.weak_signal_count}`);
  lines.push(`- live misses: ${backlog.summary.live_miss_count}`);
  lines.push(`- replay misses: ${backlog.summary.replay_miss_count}`);
  lines.push(`- recommended next signal: \`${backlog.summary.recommended_next_signal ?? 'n/a'}\``);
  lines.push('');
  lines.push('## Weak Active Signals');
  lines.push('');
  if (backlog.weak_signals.length === 0) {
    lines.push('- none');
  } else {
    for (const signal of backlog.weak_signals) {
      lines.push(
        `- \`${signal.signal_kind}\`: ${signal.recommendation} (session clean=${signal.session_clean_rate ?? 'n/a'}, trial miss=${signal.session_trial_miss_rate ?? 'n/a'}, expected missing=${signal.expected_missing_count ?? 0}, present not top=${signal.expected_present_not_top_count ?? 0}, crowded others=${signal.crowded_out_expected_count ?? 0}, unexpected top=${signal.unexpected_top_action_count ?? 0}, telemetry incomplete=${signal.telemetry_incomplete_count ?? 0}, remediation=${signal.remediation_success_rate ?? 'n/a'})`,
      );
    }
  }
  lines.push('');
  appendCandidateSection(lines, '## Active Signal Misses', backlog.active_signal_misses);
  appendCandidateSection(lines, '## Next Signal Candidates', backlog.next_signal_candidates);
  return `${lines.join('\n')}\n`;
}
