import { asArray, safeRatio } from './signal-summary-utils.mjs';

function uniqueSortedStrings(values) {
  return [...new Set(values.filter(Boolean).map(String))].sort((left, right) =>
    left.localeCompare(right),
  );
}

function focusAreasForExpectedSignals(expectedSignalKinds, tags) {
  const focusAreas = new Set();

  for (const signalKind of expectedSignalKinds) {
    if (signalKind.includes('clone')) {
      focusAreas.add('clone_followthrough');
      continue;
    }
    if (signalKind.includes('propagation') || signalKind.includes('exhaustiveness')) {
      focusAreas.add('propagation');
      continue;
    }
    if (
      signalKind.includes('boundary') ||
      signalKind.includes('raw_') ||
      signalKind.includes('writer')
    ) {
      focusAreas.add('boundary');
      continue;
    }
    focusAreas.add('structural');
  }

  for (const tag of tags) {
    if (tag === 'clone') {
      focusAreas.add('clone_followthrough');
    }
    if (tag === 'propagation') {
      focusAreas.add('propagation');
    }
    if (tag === 'governance' || tag === 'session') {
      focusAreas.add('session_governance');
    }
  }

  if (focusAreas.size === 0) {
    focusAreas.add('general');
  }

  return [...focusAreas].sort((left, right) => left.localeCompare(right));
}

function sessionIdForEntry(entry) {
  return entry.task_id ?? entry.replay_id ?? entry.commit ?? entry.task_label ?? 'session';
}

function sessionLabelForEntry(entry) {
  return entry.task_label ?? entry.commit ?? entry.replay_id ?? entry.task_id ?? 'session';
}

function normalizeInitialActionKinds(outcome) {
  return uniqueSortedStrings(asArray(outcome?.initial_action_kinds));
}

function buildEvidenceFlags(expectedSignalKinds, initialActionKinds, initialTopActionKind, outcome) {
  const expectedSignalSet = new Set(expectedSignalKinds);
  const initialActionSet = new Set(initialActionKinds);
  const expectedSignalsPresent = expectedSignalKinds.filter((kind) => initialActionSet.has(kind));
  const expectedSignalMissing =
    expectedSignalKinds.length > 0 && expectedSignalsPresent.length === 0;
  const expectedSignalPresentNotTop =
    expectedSignalsPresent.length > 0 &&
    initialTopActionKind !== null &&
    !expectedSignalSet.has(initialTopActionKind);
  const unexpectedTopAction =
    Boolean(initialTopActionKind) &&
    expectedSignalKinds.length > 0 &&
    !expectedSignalSet.has(initialTopActionKind);

  return {
    expected_signal_missing: expectedSignalMissing,
    expected_signal_present_not_top: expectedSignalPresentNotTop,
    unexpected_top_action: unexpectedTopAction,
    followup_regression_introduced: outcome?.followup_regression_introduced ?? false,
    top_action_cleared: outcome?.top_action_cleared ?? false,
    final_session_clean: outcome?.final_session_clean ?? false,
  };
}

function outcomeBucketForEntry(entry) {
  if (entry.status !== 'completed') {
    return 'provider_failed';
  }
  if (entry.evidence_flags.followup_regression_introduced) {
    return 'regressed';
  }
  if (entry.evidence_flags.expected_signal_missing) {
    return entry.outcome.final_session_clean ? 'clean_but_missed_expected_signal' : 'missed_expected_signal';
  }
  if (entry.evidence_flags.expected_signal_present_not_top) {
    return entry.outcome.final_session_clean
      ? 'clean_but_misranked'
      : 'expected_signal_present_not_top';
  }
  if (entry.outcome.convergence_status === 'thrashing') {
    return 'thrashing';
  }
  if (entry.outcome.convergence_status === 'stalled') {
    return 'stalled';
  }
  if (entry.outcome.final_session_clean) {
    return 'clean';
  }

  return 'incomplete';
}

function normalizeBatchEntry(entry, lane) {
  const outcome = entry.outcome ?? {};
  const expectedSignalKinds = uniqueSortedStrings(asArray(entry.expected_signal_kinds));
  const initialActionKinds = normalizeInitialActionKinds(outcome);
  const initialTopActionKind =
    typeof outcome.initial_top_action_kind === 'string' ? outcome.initial_top_action_kind : null;
  const tags = uniqueSortedStrings(asArray(entry.tags));
  const normalizedEntry = {
    session_id: sessionIdForEntry(entry),
    session_label: sessionLabelForEntry(entry),
    lane,
    status: entry.status ?? 'completed',
    output_dir: entry.output_dir ?? null,
    tags,
    expected_signal_kinds: expectedSignalKinds,
    expected_fix_surface: entry.expected_fix_surface ?? null,
    experiment_arm: entry.experiment_arm ?? null,
    session_goal: entry.session_goal ?? null,
    success_criteria: entry.success_criteria ?? null,
    focus_areas: focusAreasForExpectedSignals(expectedSignalKinds, tags),
    outcome: {
      session_count: outcome.session_count ?? null,
      initial_action_kinds: initialActionKinds,
      initial_top_action_kind: initialTopActionKind,
      top_action_cleared: outcome.top_action_cleared ?? false,
      checks_to_clear_top_action: outcome.checks_to_clear_top_action ?? null,
      convergence_status: outcome.convergence_status ?? null,
      entropy_delta: outcome.entropy_delta ?? null,
      final_gate: outcome.final_gate ?? null,
      final_session_clean: outcome.final_session_clean ?? false,
      followup_regression_introduced: outcome.followup_regression_introduced ?? false,
    },
  };

  normalizedEntry.evidence_flags = buildEvidenceFlags(
    normalizedEntry.expected_signal_kinds,
    normalizedEntry.outcome.initial_action_kinds,
    normalizedEntry.outcome.initial_top_action_kind,
    normalizedEntry.outcome,
  );
  normalizedEntry.outcome_bucket = outcomeBucketForEntry(normalizedEntry);

  return normalizedEntry;
}

function normalizeBatchEntries(batch, lane) {
  const results = asArray(batch?.results).map(function fromResult(result) {
    return normalizeBatchEntry(result, lane);
  });
  const failures = asArray(batch?.failures).map(function fromFailure(failure) {
    return normalizeBatchEntry(failure, lane);
  });

  return [...results, ...failures];
}

function countEntries(entries, predicate) {
  return entries.filter(predicate).length;
}

function countFocusArea(entries, focusArea) {
  return countEntries(entries, function hasFocusArea(entry) {
    return entry.focus_areas.includes(focusArea);
  });
}

function countFocusAreaBucket(entries, focusArea, outcomeBucket) {
  return countEntries(entries, function hasFocusAreaBucket(entry) {
    return entry.focus_areas.includes(focusArea) && entry.outcome_bucket === outcomeBucket;
  });
}

function isExpectedSignalEscapeBucket(outcomeBucket) {
  return (
    outcomeBucket === 'missed_expected_signal' ||
    outcomeBucket === 'clean_but_missed_expected_signal' ||
    outcomeBucket === 'expected_signal_present_not_top' ||
    outcomeBucket === 'clean_but_misranked'
  );
}

function countFocusAreaEscapes(entries, focusArea) {
  return countEntries(entries, function hasFocusAreaEscape(entry) {
    return (
      entry.focus_areas.includes(focusArea) &&
      isExpectedSignalEscapeBucket(entry.outcome_bucket)
    );
  });
}

function hasTopActionSession(entry) {
  return (
    typeof entry.outcome.initial_top_action_kind === 'string' &&
    entry.outcome.initial_top_action_kind.length > 0
  );
}

function buildCorpusSummary(entries, sessionTelemetry) {
  const liveEntries = entries.filter(function isLive(entry) {
    return entry.lane === 'live';
  });
  const replayEntries = entries.filter(function isReplay(entry) {
    return entry.lane === 'replay';
  });
  const cleanSessionCount = countEntries(entries, function isClean(entry) {
    return entry.outcome.final_session_clean;
  });
  const providerFailureCount = countEntries(entries, function isProviderFailure(entry) {
    return entry.outcome_bucket === 'provider_failed';
  });
  const regressionSessionCount = countEntries(entries, function isRegression(entry) {
    return entry.outcome_bucket === 'regressed';
  });
  const topActionSessionCount = countEntries(entries, hasTopActionSession);
  const topActionClearedCount = countEntries(entries, function hasClearedTopAction(entry) {
    return hasTopActionSession(entry) && entry.outcome.top_action_cleared === true;
  });
  const regressionAfterFixCount = countEntries(entries, function hasRegressionAfterFix(entry) {
    return (
      hasTopActionSession(entry) &&
      entry.outcome.top_action_cleared === true &&
      entry.outcome.followup_regression_introduced === true
    );
  });
  const thrashingSessionCount = countEntries(entries, function isThrashing(entry) {
    return entry.outcome_bucket === 'thrashing';
  });
  const stalledSessionCount = countEntries(entries, function isStalled(entry) {
    return entry.outcome_bucket === 'stalled';
  });
  const missedExpectedSignalCount = countEntries(entries, function isMissedExpected(entry) {
    return (
      entry.outcome_bucket === 'missed_expected_signal' ||
      entry.outcome_bucket === 'clean_but_missed_expected_signal'
    );
  });
  const misrankedExpectedSignalCount = countEntries(entries, function isMisranked(entry) {
    return (
      entry.outcome_bucket === 'expected_signal_present_not_top' ||
      entry.outcome_bucket === 'clean_but_misranked'
    );
  });

  const propagationSessionCount = countFocusArea(entries, 'propagation');
  const cloneSessionCount = countFocusArea(entries, 'clone_followthrough');

  return {
    session_count: entries.length,
    live_session_count: liveEntries.length,
    replay_session_count: replayEntries.length,
    clean_session_count: cleanSessionCount,
    provider_failure_count: providerFailureCount,
    regression_session_count: regressionSessionCount,
    top_action_session_count: topActionSessionCount,
    top_action_cleared_count: topActionClearedCount,
    regression_after_fix_count: regressionAfterFixCount,
    thrashing_session_count: thrashingSessionCount,
    stalled_session_count: stalledSessionCount,
    missed_expected_signal_count: missedExpectedSignalCount,
    expected_signal_present_not_top_count: misrankedExpectedSignalCount,
    propagation_session_count: propagationSessionCount,
    clone_session_count: cloneSessionCount,
    agent_clear_rate: safeRatio(topActionClearedCount, topActionSessionCount),
    provider_failure_rate: safeRatio(providerFailureCount, entries.length),
    regression_after_fix_rate: safeRatio(regressionAfterFixCount, topActionSessionCount),
    propagation_escape_rate: safeRatio(countFocusAreaEscapes(entries, 'propagation'), propagationSessionCount),
    duplicate_logic_introduced_rate: safeRatio(
      countEntries(entries, function hasSessionClone(entry) {
        return entry.outcome.initial_action_kinds.includes('session_introduced_clone');
      }),
      entries.length,
    ),
    clone_followthrough_escape_rate: safeRatio(
      countFocusAreaEscapes(entries, 'clone_followthrough'),
      cloneSessionCount,
    ),
    telemetry_session_count: sessionTelemetry?.summary?.session_count ?? 0,
    telemetry_thrashing_session_count:
      sessionTelemetry?.summary?.thrashing_session_count ?? 0,
    telemetry_average_entropy_delta:
      sessionTelemetry?.summary?.average_entropy_delta ?? null,
  };
}

function needsReview(entry) {
  return (
    entry.outcome_bucket === 'regressed' ||
    entry.outcome_bucket === 'thrashing' ||
    entry.outcome_bucket === 'stalled' ||
    isExpectedSignalEscapeBucket(entry.outcome_bucket)
  );
}

function selectReviewQueue(entries) {
  return entries
    .filter(needsReview)
    .sort(function compareEntries(left, right) {
      if (left.lane !== right.lane) {
        return left.lane.localeCompare(right.lane);
      }
      return left.session_id.localeCompare(right.session_id);
    });
}

export function buildSessionCorpus({
  repoLabel = null,
  repoRoot = null,
  sessionTelemetry = null,
  codexBatch = null,
  replayBatch = null,
}) {
  const entries = [
    ...normalizeBatchEntries(codexBatch, 'live'),
    ...normalizeBatchEntries(replayBatch, 'replay'),
  ].sort(function compareEntries(left, right) {
    if (left.lane !== right.lane) {
      return left.lane.localeCompare(right.lane);
    }
    return left.session_id.localeCompare(right.session_id);
  });

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_label:
      repoLabel ??
      codexBatch?.repo_label ??
      replayBatch?.repo_label ??
      sessionTelemetry?.repo_label ??
      null,
    repo_root:
      repoRoot ??
      codexBatch?.repo_root ??
      replayBatch?.repo_root ??
      sessionTelemetry?.repo_root ??
      null,
    summary: buildCorpusSummary(entries, sessionTelemetry),
    review_queue: selectReviewQueue(entries),
    sessions: entries,
  };
}

function appendSessionSection(lines, title, entries) {
  if (entries.length === 0) {
    return;
  }

  lines.push(`## ${title}`);
  lines.push('');
  for (const entry of entries) {
    lines.push(
      `- [${entry.lane}] \`${entry.session_id}\`: bucket=${entry.outcome_bucket}, expected=[${entry.expected_signal_kinds.join(', ')}], top=${entry.outcome.initial_top_action_kind ?? 'none'}, clean=${entry.outcome.final_session_clean}, regression=${entry.outcome.followup_regression_introduced}`,
    );
  }
  lines.push('');
}

export function formatSessionCorpusMarkdown(corpus) {
  const lines = [];
  lines.push('# Session Corpus');
  lines.push('');
  lines.push(`- repo: \`${corpus.repo_label ?? 'unknown'}\``);
  lines.push(`- repo root: \`${corpus.repo_root ?? 'unknown'}\``);
  lines.push(`- generated at: \`${corpus.generated_at}\``);
  lines.push(`- sessions: ${corpus.summary.session_count}`);
  lines.push(`- live sessions: ${corpus.summary.live_session_count}`);
  lines.push(`- replay sessions: ${corpus.summary.replay_session_count}`);
  lines.push(`- top-action sessions: ${corpus.summary.top_action_session_count ?? 0}`);
  lines.push(`- top actions cleared: ${corpus.summary.top_action_cleared_count ?? 0}`);
  lines.push(`- agent clear rate: ${corpus.summary.agent_clear_rate ?? 'n/a'}`);
  lines.push(
    `- regression-after-fix rate: ${corpus.summary.regression_after_fix_rate ?? 'n/a'}`,
  );
  lines.push(`- propagation escape rate: ${corpus.summary.propagation_escape_rate ?? 'n/a'}`);
  lines.push(
    `- duplicate logic introduced rate: ${corpus.summary.duplicate_logic_introduced_rate ?? 'n/a'}`,
  );
  lines.push(
    `- clone followthrough escape rate: ${corpus.summary.clone_followthrough_escape_rate ?? 'n/a'}`,
  );
  lines.push('');

  appendSessionSection(lines, 'Review Queue', corpus.review_queue.slice(0, 10));
  appendSessionSection(
    lines,
    'Propagation Examples',
    corpus.sessions
      .filter(function isPropagation(entry) {
        return entry.focus_areas.includes('propagation');
      })
      .slice(0, 10),
  );
  appendSessionSection(
    lines,
    'Clone Examples',
    corpus.sessions
      .filter(function isClone(entry) {
        return entry.focus_areas.includes('clone_followthrough');
      })
      .slice(0, 10),
  );

  return `${lines.join('\n')}\n`;
}
