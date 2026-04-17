import { ensureMapEntry, safeRatio } from './signal-summary-utils.mjs';

function countSessionsWithStatus(sessions, convergenceStatus) {
  return sessions.filter(function hasConvergenceStatus(session) {
    return session.convergence_status === convergenceStatus;
  }).length;
}

function countSessionsMatching(sessions, predicate) {
  return sessions.filter(predicate).length;
}

function totalEntropyDelta(sessions) {
  return sessions.reduce(function sumEntropyDelta(total, session) {
    return total + (session.entropy_delta ?? 0);
  }, 0);
}

function totalChecksToClear(sessions) {
  return sessions.reduce(function sumChecksToClear(total, session) {
    return total + (session.checks_to_clear_top_action ?? 0);
  }, 0);
}

function hasTopActionSession(session) {
  return (
    typeof session.initial_top_action_kind === 'string' && session.initial_top_action_kind.length > 0
  );
}

export function createSessionSignalEntry(signalKind) {
  return {
    signal_kind: signalKind,
    top_action_presented: 0,
    top_action_sessions: 0,
    followup_checks: 0,
    target_cleared: 0,
    followup_regressions: 0,
    sessions_cleared: 0,
    sessions_clean: 0,
    total_checks_to_clear: 0,
    sessions_thrashing: 0,
    sessions_stalled: 0,
    reopened_top_actions: 0,
    repeated_top_action_carries: 0,
    total_entropy_delta: 0,
    sessions_with_entropy_increase: 0,
  };
}

export function cloneSessionSignalEntry(signal) {
  return {
    signal_kind: signal.signal_kind,
    top_action_presented: signal.top_action_presented ?? 0,
    top_action_sessions: signal.top_action_sessions ?? 0,
    followup_checks: signal.followup_checks ?? 0,
    target_cleared: signal.target_cleared ?? 0,
    followup_regressions: signal.followup_regressions ?? 0,
    sessions_cleared: signal.sessions_cleared ?? 0,
    sessions_clean: signal.sessions_clean ?? 0,
    total_checks_to_clear: signal.total_checks_to_clear ?? 0,
    sessions_thrashing: signal.sessions_thrashing ?? 0,
    sessions_stalled: signal.sessions_stalled ?? 0,
    reopened_top_actions: signal.reopened_top_actions ?? 0,
    repeated_top_action_carries: signal.repeated_top_action_carries ?? 0,
    total_entropy_delta: signal.total_entropy_delta ?? 0,
    sessions_with_entropy_increase: signal.sessions_with_entropy_increase ?? 0,
  };
}

export function ensureSessionSignalEntry(signalMap, signalKind) {
  return ensureMapEntry(signalMap, signalKind, createSessionSignalEntry);
}

export function mergeSessionSignalEntry(target, source) {
  target.top_action_presented += source.top_action_presented ?? 0;
  target.top_action_sessions += source.top_action_sessions ?? 0;
  target.followup_checks += source.followup_checks ?? 0;
  target.target_cleared += source.target_cleared ?? 0;
  target.followup_regressions += source.followup_regressions ?? 0;
  target.sessions_cleared += source.sessions_cleared ?? 0;
  target.sessions_clean += source.sessions_clean ?? 0;
  target.total_checks_to_clear += source.total_checks_to_clear ?? 0;
  target.sessions_thrashing += source.sessions_thrashing ?? 0;
  target.sessions_stalled += source.sessions_stalled ?? 0;
  target.reopened_top_actions += source.reopened_top_actions ?? 0;
  target.repeated_top_action_carries += source.repeated_top_action_carries ?? 0;
  target.total_entropy_delta += source.total_entropy_delta ?? 0;
  target.sessions_with_entropy_increase += source.sessions_with_entropy_increase ?? 0;
}

export function buildSessionSignalMetrics({
  followupChecks,
  targetCleared,
  followupRegressions,
  topActionSessions,
  sessionsCleared,
  sessionsClean,
  sessionsThrashing,
  sessionsStalled,
  reopenedTopActions,
  repeatedTopActionCarries,
  totalEntropyDelta,
  sessionsWithEntropyIncrease,
  totalChecksToClear,
}) {
  const topActionClearRate = safeRatio(sessionsCleared, topActionSessions);
  const reopenedTopActionRate = safeRatio(reopenedTopActions, topActionSessions);

  return {
    resolution_rate: safeRatio(targetCleared, followupChecks),
    regression_rate: safeRatio(followupRegressions, followupChecks),
    session_resolution_rate: safeRatio(targetCleared, followupChecks),
    session_clear_rate: topActionClearRate,
    agent_clear_rate: topActionClearRate,
    top_action_clear_rate: topActionClearRate,
    followup_regression_rate: safeRatio(followupRegressions, followupChecks),
    session_clean_rate: safeRatio(sessionsClean, topActionSessions),
    session_thrash_rate: safeRatio(sessionsThrashing, topActionSessions),
    session_stall_rate: safeRatio(sessionsStalled, topActionSessions),
    reopened_top_action_rate: reopenedTopActionRate,
    regression_after_fix_rate: reopenedTopActionRate,
    repeated_top_action_carry_rate: safeRatio(repeatedTopActionCarries, topActionSessions),
    average_entropy_delta: safeRatio(totalEntropyDelta, topActionSessions),
    entropy_increase_rate: safeRatio(sessionsWithEntropyIncrease, topActionSessions),
    average_checks_to_clear: safeRatio(totalChecksToClear, sessionsCleared),
  };
}

export function buildSessionHealthSummary(sessions) {
  const summarizedSessions = Array.isArray(sessions) ? sessions : [];
  const topActionSessionCount = countSessionsMatching(
    summarizedSessions,
    hasTopActionSession,
  );
  const topActionClearedCount = countSessionsMatching(
    summarizedSessions,
    function hasClearedTopAction(session) {
      return session.top_action_cleared === true;
    },
  );
  const sessionCleanCount = countSessionsMatching(
    summarizedSessions,
    function isSessionClean(session) {
      return hasTopActionSession(session) && session.final_session_clean === true;
    },
  );
  const followupRegressionCount = countSessionsMatching(
    summarizedSessions,
    function hasFollowupRegression(session) {
      return session.followup_regression_introduced === true;
    },
  );
  const reopenedTopActionCount = countSessionsMatching(
    summarizedSessions,
    function hasReopenedTopAction(session) {
      return session.reopened_top_action === true;
    },
  );
  const entropyIncreaseSessionCount = countSessionsMatching(
    summarizedSessions,
    function hasEntropyIncrease(session) {
      return hasTopActionSession(session) && (session.entropy_delta ?? 0) > 0;
    },
  );

  return {
    converged_session_count: countSessionsWithStatus(summarizedSessions, 'converged'),
    converging_session_count: countSessionsWithStatus(summarizedSessions, 'converging'),
    stalled_session_count: countSessionsWithStatus(summarizedSessions, 'stalled'),
    thrashing_session_count: countSessionsWithStatus(summarizedSessions, 'thrashing'),
    top_action_session_count: topActionSessionCount,
    top_action_cleared_count: topActionClearedCount,
    followup_regression_count: followupRegressionCount,
    reopened_top_action_count: reopenedTopActionCount,
    session_clean_count: sessionCleanCount,
    entropy_increase_session_count: entropyIncreaseSessionCount,
    top_action_clear_rate: safeRatio(topActionClearedCount, topActionSessionCount),
    agent_clear_rate: safeRatio(topActionClearedCount, topActionSessionCount),
    followup_regression_session_rate: safeRatio(
      followupRegressionCount,
      topActionSessionCount,
    ),
    regression_after_fix_rate: safeRatio(reopenedTopActionCount, topActionSessionCount),
    session_clean_rate: safeRatio(sessionCleanCount, topActionSessionCount),
    session_thrash_rate: safeRatio(
      countSessionsWithStatus(summarizedSessions, 'thrashing'),
      topActionSessionCount,
    ),
    session_stall_rate: safeRatio(
      countSessionsWithStatus(summarizedSessions, 'stalled'),
      topActionSessionCount,
    ),
    entropy_increase_rate: safeRatio(entropyIncreaseSessionCount, topActionSessionCount),
    average_checks_to_clear: safeRatio(totalChecksToClear(summarizedSessions), topActionClearedCount),
    average_entropy_delta: safeRatio(
      totalEntropyDelta(summarizedSessions),
      summarizedSessions.length,
    ),
  };
}

export function mergeSessionHealthSummaryCounts(target, source, sessionCount) {
  target.converged_session_count += source.converged_session_count ?? 0;
  target.converging_session_count += source.converging_session_count ?? 0;
  target.stalled_session_count += source.stalled_session_count ?? 0;
  target.thrashing_session_count += source.thrashing_session_count ?? 0;
  target.top_action_session_count += source.top_action_session_count ?? 0;
  target.top_action_cleared_count += source.top_action_cleared_count ?? 0;
  target.followup_regression_count += source.followup_regression_count ?? 0;
  target.reopened_top_action_count += source.reopened_top_action_count ?? 0;
  target.session_clean_count += source.session_clean_count ?? 0;
  target.entropy_increase_session_count += source.entropy_increase_session_count ?? 0;
  target.total_checks_to_clear +=
    (source.average_checks_to_clear ?? 0) * (source.top_action_cleared_count ?? 0);
  target.total_entropy_delta += (source.average_entropy_delta ?? 0) * sessionCount;
}

export function createSessionHealthAccumulator() {
  return {
    converged_session_count: 0,
    converging_session_count: 0,
    stalled_session_count: 0,
    thrashing_session_count: 0,
    top_action_session_count: 0,
    top_action_cleared_count: 0,
    followup_regression_count: 0,
    reopened_top_action_count: 0,
    session_clean_count: 0,
    entropy_increase_session_count: 0,
    total_checks_to_clear: 0,
    total_entropy_delta: 0,
  };
}

export function finalizeSessionHealthAccumulator(accumulator, sessionCount) {
  return {
    converged_session_count: accumulator.converged_session_count,
    converging_session_count: accumulator.converging_session_count,
    stalled_session_count: accumulator.stalled_session_count,
    thrashing_session_count: accumulator.thrashing_session_count,
    top_action_session_count: accumulator.top_action_session_count,
    top_action_cleared_count: accumulator.top_action_cleared_count,
    followup_regression_count: accumulator.followup_regression_count,
    reopened_top_action_count: accumulator.reopened_top_action_count,
    session_clean_count: accumulator.session_clean_count,
    entropy_increase_session_count: accumulator.entropy_increase_session_count,
    top_action_clear_rate: safeRatio(
      accumulator.top_action_cleared_count,
      accumulator.top_action_session_count,
    ),
    agent_clear_rate: safeRatio(
      accumulator.top_action_cleared_count,
      accumulator.top_action_session_count,
    ),
    followup_regression_session_rate: safeRatio(
      accumulator.followup_regression_count,
      accumulator.top_action_session_count,
    ),
    regression_after_fix_rate: safeRatio(
      accumulator.reopened_top_action_count,
      accumulator.top_action_session_count,
    ),
    session_clean_rate: safeRatio(
      accumulator.session_clean_count,
      accumulator.top_action_session_count,
    ),
    session_thrash_rate: safeRatio(
      accumulator.thrashing_session_count,
      accumulator.top_action_session_count,
    ),
    session_stall_rate: safeRatio(
      accumulator.stalled_session_count,
      accumulator.top_action_session_count,
    ),
    entropy_increase_rate: safeRatio(
      accumulator.entropy_increase_session_count,
      accumulator.top_action_session_count,
    ),
    average_checks_to_clear: safeRatio(
      accumulator.total_checks_to_clear,
      accumulator.top_action_cleared_count,
    ),
    average_entropy_delta: safeRatio(accumulator.total_entropy_delta, sessionCount),
  };
}
