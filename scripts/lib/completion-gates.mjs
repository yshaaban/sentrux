import { asArray } from './signal-summary-utils.mjs';

const MISSING = Symbol('missing');

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function sourceLabel(condition, fallback = 'evidence') {
  return condition.source ?? fallback;
}

function splitPath(pathValue) {
  if (typeof pathValue !== 'string' || pathValue.length === 0) {
    return [];
  }

  return pathValue.split('.');
}

function readPath(source, pathValue) {
  if (pathValue === null || pathValue === undefined || pathValue === '') {
    return source ?? MISSING;
  }

  let current = source;
  for (const segment of splitPath(pathValue)) {
    if (current === null || current === undefined) {
      return MISSING;
    }

    if (Array.isArray(current) && /^\d+$/.test(segment)) {
      current = current[Number(segment)];
      continue;
    }

    current = current[segment];
  }

  return current === undefined ? MISSING : current;
}

function formatValue(value) {
  if (value === MISSING) {
    return 'missing';
  }
  if (typeof value === 'string') {
    return value;
  }
  if (value === null) {
    return 'null';
  }
  if (value === undefined) {
    return 'undefined';
  }

  return JSON.stringify(value);
}

function expectedForCondition(condition) {
  if (Object.hasOwn(condition, 'value')) {
    return condition.value;
  }
  if (Object.hasOwn(condition, 'values')) {
    return condition.values;
  }
  if (Object.hasOwn(condition, 'threshold')) {
    return condition.threshold;
  }

  return null;
}

function numeric(value) {
  return Number.isFinite(value) ? value : null;
}

function conditionPasses(operator, actual, condition) {
  const value = condition.value;
  const threshold = condition.threshold;

  switch (operator) {
    case 'exists':
      return actual !== MISSING && actual !== null;
    case 'non_empty':
      if (actual === MISSING || actual === null) {
        return false;
      }
      if (Array.isArray(actual) || typeof actual === 'string') {
        return actual.length > 0;
      }
      if (isObject(actual)) {
        return Object.keys(actual).length > 0;
      }
      return true;
    case 'is_true':
      return actual === true;
    case 'is_false':
      return actual === false;
    case 'equals':
      return actual === value;
    case 'not_equals':
      return actual !== MISSING && actual !== value;
    case 'one_of':
      return asArray(condition.values).includes(actual);
    case 'includes':
      if (Array.isArray(actual)) {
        return actual.includes(value);
      }
      if (typeof actual === 'string' && typeof value === 'string') {
        return actual.includes(value);
      }
      return false;
    case 'gte':
    case 'at_least':
      return numeric(actual) !== null && numeric(threshold) !== null && actual >= threshold;
    case 'gt':
      return numeric(actual) !== null && numeric(threshold) !== null && actual > threshold;
    case 'lte':
    case 'at_most':
      return numeric(actual) !== null && numeric(threshold) !== null && actual <= threshold;
    case 'lt':
      return numeric(actual) !== null && numeric(threshold) !== null && actual < threshold;
    default:
      throw new Error(`Unsupported completion gate operator: ${operator}`);
  }
}

function conditionMessage(condition, actual, passed) {
  if (condition.message) {
    return condition.message;
  }

  const status = passed ? 'passed' : 'failed';
  return `${condition.path ?? '(source)'} ${status}: expected ${condition.operator} ${formatValue(
    expectedForCondition(condition),
  )}, got ${formatValue(actual)}`;
}

function resolveSource(sources, condition, fallbackSource = null) {
  const name = sourceLabel(condition, fallbackSource ?? 'evidence');
  if (Object.hasOwn(sources, name)) {
    return sources[name];
  }

  return MISSING;
}

function evaluateCondition(sources, condition, fallbackSource = null) {
  const sourceName = sourceLabel(condition, fallbackSource ?? 'evidence');
  const source = resolveSource(sources, condition, fallbackSource);
  const actual = source === MISSING ? MISSING : readPath(source, condition.path);
  const operator = condition.operator ?? 'exists';
  const passed = conditionPasses(operator, actual, condition);

  return {
    gate_id: condition.gate_id ?? condition.id ?? null,
    label: condition.label ?? condition.gate_id ?? condition.id ?? condition.path ?? sourceName,
    source: sourceName,
    path: condition.path ?? null,
    operator,
    expected: expectedForCondition(condition),
    actual: actual === MISSING ? null : actual,
    status: passed ? 'pass' : 'fail',
    required: condition.required !== false,
    message: conditionMessage(condition, actual, passed),
  };
}

function requiredFailures(results) {
  return results.filter(function isRequiredFailure(result) {
    return result.required !== false && result.status === 'fail';
  });
}

function optionalFailures(results) {
  return results.filter(function isOptionalFailure(result) {
    return result.required === false && result.status === 'fail';
  });
}

function evaluatePhaseGate(sources, phase, gate) {
  const result = evaluateCondition(sources, gate);
  return {
    ...result,
    gate_id: result.gate_id ?? gate.gate_id,
    phase_id: phase.phase_id,
  };
}

function evaluatePhases(sources, rubric) {
  return asArray(rubric.phases).map(function evaluatePhase(phase) {
    const gates = asArray(phase.gates).map(function evaluateGate(gate) {
      return evaluatePhaseGate(sources, phase, gate);
    });
    const requiredFailureCount = requiredFailures(gates).length;

    return {
      phase_id: phase.phase_id,
      phase_label: phase.phase_label ?? phase.label ?? phase.phase_id,
      status: requiredFailureCount === 0 ? 'pass' : 'fail',
      required_failure_count: requiredFailureCount,
      optional_failure_count: optionalFailures(gates).length,
      gates,
    };
  });
}

function matchesSelector(signal, selector) {
  const selectorConditions = asArray(selector?.where);
  if (selectorConditions.length === 0) {
    return true;
  }

  const sources = { signal };
  return selectorConditions.every(function selectorConditionPasses(condition) {
    return evaluateCondition(sources, condition, 'signal').status === 'pass';
  });
}

function compareSignalEntries(left, right) {
  const leftKind = left.signal_kind ?? '';
  const rightKind = right.signal_kind ?? '';
  if (leftKind !== rightKind) {
    return leftKind.localeCompare(rightKind);
  }

  return (left.group_id ?? '').localeCompare(right.group_id ?? '');
}

function evaluateSignalGroups(globalSources, rubric) {
  const scorecardSignals = asArray(globalSources.scorecard?.signals);
  const entries = [];

  for (const group of asArray(rubric.signal_groups)) {
    const sourceName = group.source ?? 'scorecard';
    const source = globalSources[sourceName];
    const signals =
      sourceName === 'scorecard' && group.path === undefined
        ? scorecardSignals
        : asArray(readPath(source, group.path ?? 'signals'));

    for (const signal of signals) {
      if (!matchesSelector(signal, group.selector)) {
        continue;
      }

      const sources = {
        ...globalSources,
        signal,
      };
      const gates = asArray(group.gates).map(function evaluateSignalGate(gate) {
        return evaluateCondition(sources, gate, 'signal');
      });
      const requiredFailureCount = requiredFailures(gates).length;

      entries.push({
        group_id: group.group_id,
        group_label: group.group_label ?? group.label ?? group.group_id,
        signal_kind: signal.signal_kind ?? signal.kind ?? null,
        signal_family: signal.signal_family ?? null,
        promotion_status: signal.promotion_status ?? null,
        status: requiredFailureCount === 0 ? 'pass' : 'fail',
        required_failure_count: requiredFailureCount,
        optional_failure_count: optionalFailures(gates).length,
        gates,
      });
    }
  }

  return entries.sort(compareSignalEntries);
}

function inferRepoLabel(sources) {
  return (
    sources.scorecard?.repo_label ??
    sources.session_corpus?.repo_label ??
    sources.sessionCorpus?.repo_label ??
    sources.evidence_review?.repo_label ??
    sources.evidenceReview?.repo_label ??
    sources.backlog?.repo_label ??
    null
  );
}

function buildSummary(phases, signals) {
  const phaseFailCount = phases.filter((phase) => phase.status === 'fail').length;
  const signalFailCount = signals.filter((signal) => signal.status === 'fail').length;
  const requiredFailureCount =
    phases.reduce((total, phase) => total + phase.required_failure_count, 0) +
    signals.reduce((total, signal) => total + signal.required_failure_count, 0);
  const optionalFailureCount =
    phases.reduce((total, phase) => total + phase.optional_failure_count, 0) +
    signals.reduce((total, signal) => total + signal.optional_failure_count, 0);

  return {
    status: requiredFailureCount === 0 ? 'pass' : 'fail',
    phase_count: phases.length,
    phase_pass_count: phases.length - phaseFailCount,
    phase_fail_count: phaseFailCount,
    signal_count: signals.length,
    signal_pass_count: signals.length - signalFailCount,
    signal_fail_count: signalFailCount,
    required_failure_count: requiredFailureCount,
    optional_failure_count: optionalFailureCount,
  };
}

export function buildCompletionGates({
  rubric,
  scorecard = null,
  sessionCorpus = null,
  evidenceReview = null,
  backlog = null,
  reviewPacket = null,
  decisionRecords = null,
  generatedAt = new Date().toISOString(),
}) {
  if (!rubric || typeof rubric !== 'object') {
    throw new Error('completion gate rubric is required');
  }

  const sources = {
    rubric,
    scorecard,
    session_corpus: sessionCorpus,
    sessionCorpus,
    evidence_review: evidenceReview,
    evidenceReview,
    backlog,
    review_packet: reviewPacket,
    reviewPacket,
    decision_records: decisionRecords,
    decisionRecords,
  };
  const phases = evaluatePhases(sources, rubric);
  const signals = evaluateSignalGroups(sources, rubric);

  return {
    schema_version: 1,
    generated_at: generatedAt,
    rubric_id: rubric.rubric_id ?? null,
    rubric_version: rubric.rubric_version ?? null,
    repo_label: inferRepoLabel(sources),
    summary: buildSummary(phases, signals),
    phases,
    signals,
  };
}

export function formatCompletionGatesMarkdown(result) {
  const lines = [];
  lines.push('# Completion Gates');
  lines.push('');
  lines.push(`- status: ${result.summary.status}`);
  lines.push(`- rubric: \`${result.rubric_id ?? 'unknown'}\``);
  lines.push(`- repo: \`${result.repo_label ?? 'unknown'}\``);
  lines.push(`- generated at: \`${result.generated_at}\``);
  lines.push(`- phase gates: ${result.summary.phase_pass_count}/${result.summary.phase_count} pass`);
  lines.push(`- signal gates: ${result.summary.signal_pass_count}/${result.summary.signal_count} pass`);
  lines.push(`- required failures: ${result.summary.required_failure_count}`);
  lines.push('');

  if (result.phases.length > 0) {
    lines.push('## Phase Status');
    lines.push('');
    for (const phase of result.phases) {
      lines.push(
        `- ${phase.status.toUpperCase()} ${phase.phase_id}: ${phase.phase_label} (${phase.required_failure_count} required failure(s))`,
      );
      for (const gate of phase.gates.filter((entry) => entry.status === 'fail')) {
        const failureKind = gate.required ? 'required' : 'optional';
        lines.push(`  - ${failureKind} ${gate.gate_id ?? gate.label}: ${gate.message}`);
      }
    }
    lines.push('');
  }

  if (result.signals.length > 0) {
    lines.push('## Signal Status');
    lines.push('');
    for (const signal of result.signals) {
      lines.push(
        `- ${signal.status.toUpperCase()} ${signal.signal_kind ?? 'unknown'} (${signal.group_id})`,
      );
      for (const gate of signal.gates.filter((entry) => entry.status === 'fail')) {
        const failureKind = gate.required ? 'required' : 'optional';
        lines.push(`  - ${failureKind} ${gate.gate_id ?? gate.label}: ${gate.message}`);
      }
    }
    lines.push('');
  }

  return `${lines.join('\n')}\n`;
}
