import { readFile } from 'node:fs/promises';

function safeRatio(numerator, denominator) {
  if (!Number.isFinite(numerator) || !Number.isFinite(denominator) || denominator <= 0) {
    return null;
  }

  return Number((numerator / denominator).toFixed(3));
}

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

function createSignalEntry(signalKind) {
  return {
    signal_kind: signalKind,
    top_action_presented: 0,
    followup_checks: 0,
    target_cleared: 0,
    followup_regressions: 0,
  };
}

function ensureSignalEntry(signalMap, signalKind) {
  if (!signalMap.has(signalKind)) {
    signalMap.set(signalKind, createSignalEntry(signalKind));
  }

  return signalMap.get(signalKind);
}

function summarizeSession(sessionRunId, events, signalMap) {
  const sessionStarted = events.some((event) => event.event_type === 'session_started');
  const sessionEnded = events.some((event) => event.event_type === 'session_ended');
  const sessionEndEvent = findLastEventByType(events, 'session_ended');
  const decision = sessionEndEvent?.decision ?? null;
  const checkRuns = events.filter((event) => event.event_type === 'check_run');

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
    }
  }

  return {
    session_run_id: sessionRunId,
    session_mode: events[0]?.session_mode ?? 'implicit',
    session_started: sessionStarted,
    session_ended: sessionEnded,
    final_decision: decision,
    check_run_count: checkRuns.length,
    top_action_kinds: checkRuns
      .map((event) => event.top_action_kind)
      .filter(Boolean),
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
  lines.push('| Signal | Top Action Presented | Follow-up Checks | Target Cleared | Follow-up Regressions | Resolution Rate | Regression Rate |');
  lines.push('| --- | --- | --- | --- | --- | --- | --- |');

  for (const signal of summary.signals) {
    lines.push(
      `| \`${signal.signal_kind}\` | ${signal.top_action_presented} | ${signal.followup_checks} | ${signal.target_cleared} | ${signal.followup_regressions} | ${signal.resolution_rate ?? 'n/a'} | ${signal.regression_rate ?? 'n/a'} |`,
    );
  }

  lines.push('');
  return `${lines.join('\n')}\n`;
}
