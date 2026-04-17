import { ensureMapEntry, safeRatio } from './signal-summary-utils.mjs';

function countSessionsWithStatus(sessions, convergenceStatus) {
  return sessions.filter(function hasConvergenceStatus(session) {
    return session.convergence_status === convergenceStatus;
  }).length;
}

function totalEntropyDelta(sessions) {
  return sessions.reduce(function sumEntropyDelta(total, session) {
    return total + (session.entropy_delta ?? 0);
  }, 0);
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

  return {
    resolution_rate: safeRatio(targetCleared, followupChecks),
    regression_rate: safeRatio(followupRegressions, followupChecks),
    session_resolution_rate: safeRatio(targetCleared, followupChecks),
    session_clear_rate: topActionClearRate,
    top_action_clear_rate: topActionClearRate,
    followup_regression_rate: safeRatio(followupRegressions, followupChecks),
    session_clean_rate: safeRatio(sessionsClean, topActionSessions),
    session_thrash_rate: safeRatio(sessionsThrashing, topActionSessions),
    session_stall_rate: safeRatio(sessionsStalled, topActionSessions),
    reopened_top_action_rate: safeRatio(reopenedTopActions, topActionSessions),
    repeated_top_action_carry_rate: safeRatio(repeatedTopActionCarries, topActionSessions),
    average_entropy_delta: safeRatio(totalEntropyDelta, topActionSessions),
    entropy_increase_rate: safeRatio(sessionsWithEntropyIncrease, topActionSessions),
    average_checks_to_clear: safeRatio(totalChecksToClear, sessionsCleared),
  };
}

export function buildSessionHealthSummary(sessions) {
  const summarizedSessions = Array.isArray(sessions) ? sessions : [];

  return {
    converged_session_count: countSessionsWithStatus(summarizedSessions, 'converged'),
    converging_session_count: countSessionsWithStatus(summarizedSessions, 'converging'),
    stalled_session_count: countSessionsWithStatus(summarizedSessions, 'stalled'),
    thrashing_session_count: countSessionsWithStatus(summarizedSessions, 'thrashing'),
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
  target.total_entropy_delta += (source.average_entropy_delta ?? 0) * sessionCount;
}

export function createSessionHealthAccumulator() {
  return {
    converged_session_count: 0,
    converging_session_count: 0,
    stalled_session_count: 0,
    thrashing_session_count: 0,
    total_entropy_delta: 0,
  };
}

export function finalizeSessionHealthAccumulator(accumulator, sessionCount) {
  return {
    converged_session_count: accumulator.converged_session_count,
    converging_session_count: accumulator.converging_session_count,
    stalled_session_count: accumulator.stalled_session_count,
    thrashing_session_count: accumulator.thrashing_session_count,
    average_entropy_delta: safeRatio(accumulator.total_entropy_delta, sessionCount),
  };
}
