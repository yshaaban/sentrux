import { readFile, writeFile } from 'node:fs/promises';
import { ensureMapEntry, safeRatio } from './signal-summary-utils.mjs';

function parseJsonLine(line) {
  if (!line.trim()) {
    return null;
  }

  return JSON.parse(line);
}

function sortEvents(events) {
  return [...events].sort((left, right) => {
    const leftSession = String(left.session_run_id ?? '');
    const rightSession = String(right.session_run_id ?? '');
    if (leftSession !== rightSession) {
      return leftSession.localeCompare(rightSession);
    }

    return (left.event_index ?? 0) - (right.event_index ?? 0);
  });
}

function groupEventsBySession(events) {
  const sessions = new Map();

  for (const event of sortEvents(events)) {
    const key = String(event.session_run_id ?? event.server_run_id ?? 'unknown');
    if (!sessions.has(key)) {
      sessions.set(key, []);
    }
    sessions.get(key).push(event);
  }

  return sessions;
}

function findLastEventByType(events, eventType) {
  return [...events].reverse().find((event) => event.event_type === eventType) ?? null;
}

function countEventsByType(events, eventType) {
  return events.filter((event) => event.event_type === eventType).length;
}

function actionKindsForRun(run) {
  return Array.isArray(run?.action_kinds) ? run.action_kinds : [];
}

function firstSurfacedTopAction(checkRuns) {
  for (let index = 0; index < checkRuns.length; index += 1) {
    const topActionKind = checkRuns[index]?.top_action_kind ?? null;
    if (topActionKind) {
      return {
        index,
        kind: topActionKind,
      };
    }
  }

  return null;
}

function initialActionKindsForSession(checkRuns, firstTopAction) {
  const actionSourceIndex = firstTopAction?.index ?? 0;
  const actionSourceRun = checkRuns[actionSourceIndex] ?? null;
  return actionKindsForRun(actionSourceRun);
}

function finalGateForSession(checkRuns, sessionEndEvent) {
  if (sessionEndEvent?.decision) {
    return sessionEndEvent.decision;
  }

  return checkRuns.at(-1)?.gate ?? null;
}

function isFinalSessionClean(checkRuns, sessionEndEvent) {
  if (sessionEndEvent) {
    return sessionEndEvent.decision === 'pass' && (sessionEndEvent.action_count ?? 0) === 0;
  }

  const lastCheck = checkRuns.at(-1);
  if (!lastCheck) {
    return false;
  }

  return lastCheck.gate === 'pass' && actionKindsForRun(lastCheck).length === 0;
}

function checksUntilActionClears(topAction, checkRuns) {
  if (!topAction?.kind) {
    return null;
  }

  for (let index = topAction.index + 1; index < checkRuns.length; index += 1) {
    if (!actionKindsForRun(checkRuns[index]).includes(topAction.kind)) {
      return index - topAction.index;
    }
  }

  return null;
}

function createSignalEntry(signalKind) {
  return {
    signal_kind: signalKind,
    top_action_presented: 0,
    followup_checks: 0,
    target_cleared: 0,
    followup_regressions: 0,
    sessions_cleared: 0,
    sessions_clean: 0,
    total_checks_to_clear: 0,
  };
}

function cloneSignalEntry(signal) {
  return {
    signal_kind: signal.signal_kind,
    top_action_presented: signal.top_action_presented ?? 0,
    followup_checks: signal.followup_checks ?? 0,
    target_cleared: signal.target_cleared ?? 0,
    followup_regressions: signal.followup_regressions ?? 0,
    sessions_cleared: signal.sessions_cleared ?? 0,
    sessions_clean: signal.sessions_clean ?? 0,
    total_checks_to_clear: signal.total_checks_to_clear ?? 0,
  };
}

function ensureSignalEntry(signalMap, signalKind) {
  return ensureMapEntry(signalMap, signalKind, createSignalEntry);
}

function summarizeSession(sessionRunId, events, signalMap) {
  const sessionStarted = events.some((event) => event.event_type === 'session_started');
  const sessionEnded = events.some((event) => event.event_type === 'session_ended');
  const sessionEndEvent = findLastEventByType(events, 'session_ended');
  const decision = sessionEndEvent?.decision ?? null;
  const checkRuns = events.filter((event) => event.event_type === 'check_run');
  const initialCheck = checkRuns[0] ?? null;
  const firstTopAction = firstSurfacedTopAction(checkRuns);
  const initialActionKinds = initialActionKindsForSession(checkRuns, firstTopAction);
  const initialTopActionKind = firstTopAction?.kind ?? null;
  const checksToClearTopAction = checksUntilActionClears(firstTopAction, checkRuns);
  const topActionCleared = checksToClearTopAction !== null;
  const finalGate = finalGateForSession(checkRuns, sessionEndEvent);
  const finalSessionClean = isFinalSessionClean(checkRuns, sessionEndEvent);
  let followupRegressionIntroduced = false;

  for (let index = 0; index < checkRuns.length; index += 1) {
    const currentRun = checkRuns[index];
    const nextRun = checkRuns[index + 1] ?? null;
    const currentTopActionKind = currentRun.top_action_kind ?? null;
    if (!currentTopActionKind) {
      continue;
    }

    const entry = ensureSignalEntry(signalMap, currentTopActionKind);
    entry.top_action_presented += 1;

    if (!nextRun) {
      continue;
    }

    entry.followup_checks += 1;
    const nextActionKinds = actionKindsForRun(nextRun);
    if (!nextActionKinds.includes(currentTopActionKind)) {
      entry.target_cleared += 1;
    }
    const currentActionKinds = new Set(actionKindsForRun(currentRun));
    const introducedKinds = nextActionKinds.filter((kind) => !currentActionKinds.has(kind));
    if (introducedKinds.length > 0) {
      entry.followup_regressions += 1;
      followupRegressionIntroduced = true;
    }
  }

  if (initialTopActionKind) {
    const entry = ensureSignalEntry(signalMap, initialTopActionKind);
    if (topActionCleared) {
      entry.sessions_cleared += 1;
      entry.total_checks_to_clear += checksToClearTopAction;
    }
    if (finalSessionClean) {
      entry.sessions_clean += 1;
    }
  }

  return {
    session_run_id: sessionRunId,
    session_mode: events[0]?.session_mode ?? 'implicit',
    session_started: sessionStarted,
    session_ended: sessionEnded,
    initial_gate: initialCheck?.gate ?? null,
    initial_action_kinds: initialActionKinds,
    initial_top_action_kind: initialTopActionKind,
    top_action_cleared: topActionCleared,
    checks_to_clear_top_action: checksToClearTopAction,
    followup_regression_introduced: followupRegressionIntroduced,
    final_decision: decision,
    final_gate: finalGate,
    final_session_clean: finalSessionClean,
    check_run_count: checkRuns.length,
    top_action_kinds: checkRuns
      .map((event) => event.top_action_kind)
      .filter(Boolean),
  };
}

export function summarizeOutcome(sessionTelemetry) {
  const lastSession = sessionTelemetry.sessions.at(-1) ?? null;

  return {
    session_count: sessionTelemetry.summary.session_count,
    initial_action_kinds: lastSession?.initial_action_kinds ?? [],
    initial_top_action_kind: lastSession?.initial_top_action_kind ?? null,
    top_action_cleared: lastSession?.top_action_cleared ?? false,
    checks_to_clear_top_action: lastSession?.checks_to_clear_top_action ?? null,
    followup_regression_introduced: lastSession?.followup_regression_introduced ?? false,
    final_gate: lastSession?.final_gate ?? null,
    final_session_clean: lastSession?.final_session_clean ?? false,
  };
}

export async function readSessionTelemetryLog(targetPath) {
  const source = await readFile(targetPath, 'utf8');
  return source
    .split(/\r?\n/)
    .map(parseJsonLine)
    .filter(Boolean);
}

export async function loadSessionTelemetrySummary(sessionEventsPath, overrides = {}) {
  const events = await readSessionTelemetryLog(sessionEventsPath);
  return buildSessionTelemetrySummary(events, {
    ...overrides,
    repoRoot: overrides.repoRoot ?? events[0]?.repo_root ?? null,
    sourcePath: sessionEventsPath,
  });
}

export function createEmptySessionTelemetrySummary(repoRoot = null) {
  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_root: repoRoot,
    source_path: null,
    summary: {
      event_count: 0,
      session_count: 0,
      explicit_session_count: 0,
      implicit_session_count: 0,
      check_run_count: 0,
    },
    sessions: [],
    signals: [],
  };
}

export async function loadSessionTelemetrySummaryOrEmpty(
  sessionEventsPath,
  overrides = {},
) {
  try {
    return await loadSessionTelemetrySummary(sessionEventsPath, overrides);
  } catch (error) {
    if (error && typeof error === 'object' && error.code === 'ENOENT') {
      return createEmptySessionTelemetrySummary(overrides.repoRoot ?? null);
    }
    throw error;
  }
}

export function buildSessionTelemetrySummary(events, overrides = {}) {
  const sessions = groupEventsBySession(events);
  const signalMap = new Map();
  const summarizedSessions = [...sessions.entries()].map(([sessionRunId, sessionEvents]) =>
    summarizeSession(sessionRunId, sessionEvents, signalMap),
  );

  const signals = [...signalMap.values()]
    .map((entry) => ({
      ...entry,
      resolution_rate: safeRatio(entry.target_cleared, entry.followup_checks),
      regression_rate: safeRatio(entry.followup_regressions, entry.followup_checks),
      session_clear_rate: safeRatio(entry.sessions_cleared, entry.top_action_presented),
      session_clean_rate: safeRatio(entry.sessions_clean, entry.top_action_presented),
      average_checks_to_clear: safeRatio(entry.total_checks_to_clear, entry.sessions_cleared),
    }))
    .sort((left, right) => left.signal_kind.localeCompare(right.signal_kind));

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_root: overrides.repoRoot ?? events[0]?.repo_root ?? null,
    source_path: overrides.sourcePath ?? null,
    summary: {
      event_count: events.length,
      session_count: summarizedSessions.length,
      explicit_session_count: summarizedSessions.filter((session) => session.session_mode === 'explicit').length,
      implicit_session_count: summarizedSessions.filter((session) => session.session_mode !== 'explicit').length,
      check_run_count: countEventsByType(events, 'check_run'),
    },
    sessions: summarizedSessions,
    signals,
  };
}

export function mergeSessionTelemetrySummaries(summaries, overrides = {}) {
  const mergedSignals = new Map();
  const sessions = [];
  let eventCount = 0;
  let explicitSessionCount = 0;
  let implicitSessionCount = 0;
  let checkRunCount = 0;

  for (const summary of summaries ?? []) {
    if (!summary) {
      continue;
    }

    eventCount += summary.summary?.event_count ?? 0;
    explicitSessionCount += summary.summary?.explicit_session_count ?? 0;
    implicitSessionCount += summary.summary?.implicit_session_count ?? 0;
    checkRunCount += summary.summary?.check_run_count ?? 0;
    sessions.push(...(summary.sessions ?? []));

    for (const signal of summary.signals ?? []) {
      const signalKind = signal.signal_kind;
      if (!signalKind) {
        continue;
      }

      if (!mergedSignals.has(signalKind)) {
        mergedSignals.set(signalKind, cloneSignalEntry(signal));
        continue;
      }

      const entry = mergedSignals.get(signalKind);
      entry.top_action_presented += signal.top_action_presented ?? 0;
      entry.followup_checks += signal.followup_checks ?? 0;
      entry.target_cleared += signal.target_cleared ?? 0;
      entry.followup_regressions += signal.followup_regressions ?? 0;
      entry.sessions_cleared += signal.sessions_cleared ?? 0;
      entry.sessions_clean += signal.sessions_clean ?? 0;
      entry.total_checks_to_clear += signal.total_checks_to_clear ?? 0;
    }
  }

  const signals = [...mergedSignals.values()]
    .map((entry) => ({
      ...entry,
      resolution_rate: safeRatio(entry.target_cleared, entry.followup_checks),
      regression_rate: safeRatio(entry.followup_regressions, entry.followup_checks),
      session_clear_rate: safeRatio(entry.sessions_cleared, entry.top_action_presented),
      session_clean_rate: safeRatio(entry.sessions_clean, entry.top_action_presented),
      average_checks_to_clear: safeRatio(entry.total_checks_to_clear, entry.sessions_cleared),
    }))
    .sort((left, right) => left.signal_kind.localeCompare(right.signal_kind));

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_root: overrides.repoRoot ?? summaries?.find((summary) => summary?.repo_root)?.repo_root ?? null,
    source_path: overrides.sourcePath ?? null,
    source_paths: overrides.sourcePaths ?? [],
    summary: {
      event_count: eventCount,
      session_count: sessions.length,
      explicit_session_count: explicitSessionCount,
      implicit_session_count: implicitSessionCount,
      check_run_count: checkRunCount,
    },
    sessions,
    signals,
  };
}

export function formatSessionTelemetrySummaryMarkdown(summary) {
  const lines = [];
  lines.push('# Session Telemetry Summary');
  lines.push('');
  lines.push(`- repo root: \`${summary.repo_root ?? 'unknown'}\``);
  lines.push(`- generated at: \`${summary.generated_at}\``);
  lines.push(`- events: ${summary.summary.event_count}`);
  lines.push(`- sessions: ${summary.summary.session_count}`);
  lines.push(`- explicit sessions: ${summary.summary.explicit_session_count}`);
  lines.push(`- implicit sessions: ${summary.summary.implicit_session_count}`);
  lines.push(`- check runs: ${summary.summary.check_run_count}`);
  lines.push('');
  lines.push('| Signal | Top Action Presented | Follow-up Checks | Target Cleared | Follow-up Regressions | Resolution Rate | Regression Rate | Session Clean Rate | Avg Checks To Clear |');
  lines.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- |');

  for (const signal of summary.signals) {
    lines.push(
      `| \`${signal.signal_kind}\` | ${signal.top_action_presented} | ${signal.followup_checks} | ${signal.target_cleared} | ${signal.followup_regressions} | ${signal.resolution_rate ?? 'n/a'} | ${signal.regression_rate ?? 'n/a'} | ${signal.session_clean_rate ?? 'n/a'} | ${signal.average_checks_to_clear ?? 'n/a'} |`,
    );
  }

  lines.push('');
  return `${lines.join('\n')}\n`;
}

export async function writeSessionTelemetryArtifacts({
  telemetryJsonPath,
  telemetryMarkdownPath,
  summary,
}) {
  await writeFile(telemetryJsonPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
  await writeFile(
    telemetryMarkdownPath,
    formatSessionTelemetrySummaryMarkdown(summary),
    'utf8',
  );
}
