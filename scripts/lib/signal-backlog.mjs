function safeRatio(numerator, denominator) {
  if (!Number.isFinite(numerator) || !Number.isFinite(denominator) || denominator <= 0) {
    return null;
  }

  return Number((numerator / denominator).toFixed(3));
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function isKeepRecommendation(value) {
  return typeof value === 'string' && value.startsWith('keep_');
}

function summarizeWeakSignals(cohort, scorecard) {
  const activeSignalKinds = new Set(cohort.signals.map((signal) => signal.signal_kind));

  return asArray(scorecard?.signals)
    .filter((signal) => activeSignalKinds.has(signal.signal_kind))
    .filter((signal) => !isKeepRecommendation(signal.promotion_recommendation))
    .map((signal) => ({
      signal_kind: signal.signal_kind,
      promotion_status: signal.promotion_status,
      recommendation: signal.promotion_recommendation,
      seeded_recall: signal.seeded_recall ?? null,
      reviewed_precision: signal.reviewed_precision ?? null,
      remediation_success_rate: signal.remediation_success_rate ?? null,
      session_resolution_rate: signal.session_resolution_rate ?? null,
      session_clean_rate: signal.session_clean_rate ?? null,
      average_checks_to_clear: signal.average_checks_to_clear ?? null,
    }));
}

function createCandidateEntry(signalKind) {
  return {
    signal_kind: signalKind,
    miss_count: 0,
    replay_miss_count: 0,
    live_miss_count: 0,
    regression_followup_count: 0,
    not_in_active_cohort: 0,
  };
}

function recordCandidate(candidateMap, signalKind, bucket, activeSignalKinds) {
  if (!signalKind) {
    return;
  }

  if (!candidateMap.has(signalKind)) {
    candidateMap.set(signalKind, createCandidateEntry(signalKind));
  }

  const entry = candidateMap.get(signalKind);
  entry.miss_count += 1;
  entry[`${bucket}_miss_count`] += 1;
  if (!activeSignalKinds.has(signalKind)) {
    entry.not_in_active_cohort += 1;
  }
}

function buildMissEntries(results, lane, activeSignalKinds, candidateMap) {
  const misses = [];

  for (const result of results) {
    const outcome = result.outcome ?? {};
    const expectedKinds = asArray(result.expected_signal_kinds);
    const initialTopActionKind = outcome.initial_top_action_kind ?? null;
    const initialActionKinds = new Set(asArray(outcome.initial_action_kinds));
    const missingExpectedKinds = expectedKinds.filter((kind) => !initialActionKinds.has(kind));
    const missedExpectedSignal = expectedKinds.length > 0 && missingExpectedKinds.length > 0;
    const needsAttention =
      !outcome.final_session_clean ||
      outcome.followup_regression_introduced ||
      (missedExpectedSignal && outcome.final_session_clean === false);

    if (!needsAttention) {
      continue;
    }

    for (const signalKind of missingExpectedKinds) {
      recordCandidate(candidateMap, signalKind, lane, activeSignalKinds);
    }

    if (outcome.followup_regression_introduced && initialTopActionKind) {
      if (!candidateMap.has(initialTopActionKind)) {
        candidateMap.set(initialTopActionKind, createCandidateEntry(initialTopActionKind));
      }
      candidateMap.get(initialTopActionKind).regression_followup_count += 1;
    }

    misses.push({
      id: result.task_id ?? result.replay_id ?? null,
      lane,
      label: result.task_label ?? result.commit ?? null,
      expected_signal_kinds: expectedKinds,
      initial_action_kinds: [...initialActionKinds],
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
    .sort((left, right) => {
      if (right.not_in_active_cohort !== left.not_in_active_cohort) {
        return right.not_in_active_cohort - left.not_in_active_cohort;
      }
      if (right.miss_count !== left.miss_count) {
        return right.miss_count - left.miss_count;
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
        `- \`${candidate.signal_kind}\`: misses=${candidate.miss_count}, live=${candidate.live_miss_count}, replay=${candidate.replay_miss_count}, regression_followups=${candidate.regression_followup_count}`,
      );
    }
  }

  lines.push('');
}

export function buildSignalBacklog({ cohort, scorecard, codexBatch = null, replayBatch = null }) {
  const activeSignalKinds = new Set(cohort.signals.map((signal) => signal.signal_kind));
  const candidateMap = new Map();
  const weakSignals = summarizeWeakSignals(cohort, scorecard);
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
  const candidateSignals = summarizeNextCandidates(candidateMap);
  const nextCandidates = candidateSignals.filter((candidate) => candidate.not_in_active_cohort > 0);
  const activeSignalMisses = candidateSignals.filter((candidate) => candidate.not_in_active_cohort === 0);

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    cohort_id: cohort.cohort_id,
    active_signal_kinds: cohort.signals.map((signal) => signal.signal_kind),
    summary: {
      weak_signal_count: weakSignals.length,
      live_miss_count: liveMisses.length,
      replay_miss_count: replayMisses.length,
      next_candidate_count: nextCandidates.length,
      recommended_next_signal: nextCandidates[0]?.signal_kind ?? null,
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
        `- \`${signal.signal_kind}\`: ${signal.recommendation} (session clean=${signal.session_clean_rate ?? 'n/a'}, remediation=${signal.remediation_success_rate ?? 'n/a'})`,
      );
    }
  }
  lines.push('');
  appendCandidateSection(lines, '## Active Signal Misses', backlog.active_signal_misses);
  appendCandidateSection(lines, '## Next Signal Candidates', backlog.next_signal_candidates);
  return `${lines.join('\n')}\n`;
}
