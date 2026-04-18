import { fail } from '../common.mjs';
import { defaultChecksForTask } from './task-schemas.mjs';

function getValueAtPath(value, pathExpression) {
  if (typeof pathExpression !== 'string' || !pathExpression) {
    return undefined;
  }

  const normalized = pathExpression.replace(/^\$\.?/, '');
  if (!normalized) {
    return value;
  }

  return normalized.split('.').reduce((current, segment) => {
    if (current === null || current === undefined) {
      return undefined;
    }
    if (Array.isArray(current) && /^\d+$/.test(segment)) {
      return current[Number(segment)];
    }
    return current[segment];
  }, value);
}

function summarizeValue(value) {
  if (value === null) {
    return 'null';
  }
  if (value === undefined) {
    return 'undefined';
  }
  if (typeof value === 'string') {
    return value.length <= 180 ? value : `${value.slice(0, 177)}...`;
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  if (Array.isArray(value)) {
    const preview = value.slice(0, 3).map((entry) => summarizeValue(entry));
    return `array(len=${value.length}, preview=${JSON.stringify(preview)})`;
  }
  if (typeof value === 'object') {
    const keys = Object.keys(value);
    return `object(keys=${keys.length}, preview=${JSON.stringify(keys.slice(0, 6))})`;
  }
  return String(value);
}

function summarizeArrayLength(value) {
  if (Array.isArray(value)) {
    return `array(len=${value.length})`;
  }

  return summarizeValue(value);
}

function checksForTask(task) {
  if (Array.isArray(task.checks) && task.checks.length > 0) {
    return task.checks;
  }

  return defaultChecksForTask(task);
}

function evaluationSummaryForStatus(status) {
  switch (status) {
    case 'pass':
      return 'all required checks passed';
    case 'warn':
      return 'required checks passed, optional checks failed';
    default:
      return 'provider or required checks failed';
  }
}

function runCheck(responseJson, check) {
  const severity = check.severity ?? 'required';
  const observed = getValueAtPath(responseJson, check.path);
  const result = {
    kind: check.kind,
    path: check.path,
    severity,
    passed: false,
    observed_summary: summarizeValue(observed),
    message: '',
  };

  switch (check.kind) {
    case 'has': {
      result.passed = observed !== undefined && observed !== null;
      result.message = result.passed ? 'value present' : 'value missing';
      break;
    }
    case 'truthy': {
      result.passed = Boolean(observed);
      result.message = result.passed ? 'value truthy' : 'value falsey';
      break;
    }
    case 'min_items': {
      result.passed = Array.isArray(observed) && observed.length >= Number(check.min ?? 0);
      result.message = result.passed ? `length >= ${check.min}` : `length < ${check.min}`;
      result.observed_summary = summarizeArrayLength(observed);
      break;
    }
    case 'max_items': {
      result.passed = Array.isArray(observed) && observed.length <= Number(check.max ?? 0);
      result.message = result.passed ? `length <= ${check.max}` : `length > ${check.max}`;
      result.observed_summary = summarizeArrayLength(observed);
      break;
    }
    case 'enum': {
      const allowed = Array.isArray(check.allowed) ? check.allowed : [];
      result.passed = allowed.some((candidate) => candidate === observed);
      result.message = result.passed ? 'value matched enum' : 'value not in enum';
      result.expected_summary = `one_of(${JSON.stringify(allowed)})`;
      break;
    }
    case 'number_gte': {
      result.passed = typeof observed === 'number' && observed >= Number(check.min);
      result.message = result.passed ? `value >= ${check.min}` : `value < ${check.min}`;
      result.expected_summary = `>= ${check.min}`;
      break;
    }
    case 'number_lte': {
      result.passed = typeof observed === 'number' && observed <= Number(check.max);
      result.message = result.passed ? `value <= ${check.max}` : `value > ${check.max}`;
      result.expected_summary = `<= ${check.max}`;
      break;
    }
    case 'contains_text': {
      const needle = typeof check.value === 'string' ? check.value : '';
      if (typeof observed === 'string') {
        result.passed = observed.includes(needle);
      } else if (Array.isArray(observed)) {
        result.passed = observed.some(
          (entry) => typeof entry === 'string' && entry.includes(needle),
        );
      }
      result.message = result.passed ? 'text matched' : 'text missing';
      result.expected_summary = `contains(${JSON.stringify(needle)})`;
      break;
    }
    case 'matches': {
      const pattern = typeof check.pattern === 'string' ? check.pattern : '';
      const flags = typeof check.flags === 'string' ? check.flags : '';
      const regex = new RegExp(pattern, flags);
      result.passed = typeof observed === 'string' && regex.test(observed);
      result.message = result.passed ? 'pattern matched' : 'pattern missing';
      result.expected_summary = `/${pattern}/${flags}`;
      break;
    }
    case 'equals': {
      result.passed = JSON.stringify(observed) === JSON.stringify(check.value);
      result.message = result.passed ? 'value matched' : 'value mismatch';
      result.expected_summary = summarizeValue(check.value);
      break;
    }
    case 'all_items_in_set': {
      const allowed = new Set(Array.isArray(check.allowed) ? check.allowed : []);
      result.passed =
        observed === undefined ||
        observed === null ||
        (Array.isArray(observed) &&
          observed.every((entry) => typeof entry === 'string' && allowed.has(entry)));
      result.message = result.passed ? 'all items matched allowed set' : 'items outside allowed set';
      result.expected_summary = `subset_of(${JSON.stringify([...allowed])})`;
      result.observed_summary = Array.isArray(observed)
        ? `array(len=${observed.length}, preview=${JSON.stringify(observed.slice(0, 5))})`
        : summarizeValue(observed);
      break;
    }
    default:
      fail(`Unsupported check kind: ${check.kind}`);
  }

  if (!result.expected_summary && Object.prototype.hasOwnProperty.call(check, 'value')) {
    result.expected_summary = summarizeValue(check.value);
  }

  return result;
}

function evaluateTask(task, responseJson, providerStatus) {
  const checks = checksForTask(task);
  const checkResults = checks.map((check) => runCheck(responseJson, check));
  const requiredChecks = checkResults.filter((check) => check.severity !== 'optional');
  const requiredFailures = requiredChecks.filter((check) => !check.passed);
  const optionalFailures = checkResults.filter(
    (check) => check.severity === 'optional' && !check.passed,
  );
  const providerFailed =
    providerStatus.exit_code !== 0 || providerStatus.timed_out || !providerStatus.stdout_json;

  let status = 'pass';
  if (providerFailed || requiredFailures.length > 0) {
    status = 'fail';
  } else if (optionalFailures.length > 0) {
    status = 'warn';
  }

  const passedCount = checkResults.filter((check) => check.passed).length;
  const score_0_100 = requiredChecks.length
    ? Math.max(0, Math.min(100, Math.round((passedCount / checkResults.length) * 100)))
    : 0;

  return {
    status,
    score_0_100,
    required_check_count: requiredChecks.length,
    passed_check_count: passedCount,
    failed_check_count: checkResults.length - passedCount,
    check_results: checkResults,
    provider_failed: providerFailed,
    summary: evaluationSummaryForStatus(status),
  };
}

function parseMaybeJson(text) {
  if (typeof text !== 'string') {
    return null;
  }

  const trimmed = text.trim();
  if (!trimmed) {
    return null;
  }

  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

function extractResponsePayload(providerOutput, task) {
  const outer = providerOutput.stdout_json;
  if (!outer) {
    return {
      parse_status: 'stdout_not_json',
      response_json: null,
      response_text: providerOutput.stdout.trim() || null,
      outer_json: null,
    };
  }

  if (outer && typeof outer === 'object' && 'result' in outer) {
    const result = outer.result;
    if (result && typeof result === 'object') {
      return {
        parse_status: 'outer_result_object',
        response_json: result,
        response_text: JSON.stringify(result, null, 2),
        outer_json: outer,
      };
    }
    if (typeof result === 'string') {
      const parsed = parseMaybeJson(result);
      return {
        parse_status: parsed ? 'outer_result_json' : 'outer_result_text',
        response_json: parsed,
        response_text: result,
        outer_json: outer,
      };
    }
  }

  const directJson = outer;
  const looksLikeTaskPayload =
    directJson &&
    typeof directJson === 'object' &&
    typeof directJson.task_kind === 'string' &&
    ((task.kind === 'agent_brief' && directJson.task_kind === 'agent_brief') ||
      (task.kind === 'dead_private' && directJson.task_kind === 'dead_private') ||
      (task.kind === 'bounded_adjudication' &&
        directJson.task_kind === 'bounded_adjudication'));

  if (looksLikeTaskPayload) {
    return {
      parse_status: 'direct_json_object',
      response_json: directJson,
      response_text: JSON.stringify(directJson, null, 2),
      outer_json: outer,
    };
  }

  return {
    parse_status: 'outer_json_unrecognized',
    response_json: null,
    response_text: JSON.stringify(outer, null, 2),
    outer_json: outer,
  };
}

export {
  evaluateTask,
  extractResponsePayload,
  getValueAtPath,
  parseMaybeJson,
  runCheck,
  summarizeValue,
};
