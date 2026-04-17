import { runClaudeCode } from '../../evals/providers/claude-code.mjs';
import { runCodexExec } from '../../evals/providers/codex-cli.mjs';
import { fail, nowIso } from './common.mjs';
import {
  assertScenarioRepoExists,
  buildScenarioSummary,
  resolveRepoRoot,
} from './scenarios.mjs';

const BASE_APPEND_SYSTEM_PROMPT = [
  'You are an external evaluation worker.',
  'Return only the JSON object that matches the schema passed on the command line.',
  'Do not edit files.',
  'If evidence is uncertain, say so directly and lower confidence instead of speculating.',
].join(' ');

const AGENT_BRIEF_OUTPUT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['task_kind', 'repo_name', 'mode', 'summary', 'top_signals', 'next_steps', 'confidence_0_1'],
  properties: {
    task_kind: { const: 'agent_brief' },
    repo_name: { type: 'string', minLength: 1 },
    mode: { enum: ['repo_onboarding', 'patch', 'pre_merge'] },
    summary: { type: 'string', minLength: 1 },
    top_signals: {
      type: 'array',
      minItems: 1,
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['title', 'summary'],
        properties: {
          title: { type: 'string', minLength: 1 },
          summary: { type: 'string', minLength: 1 },
          kind: { type: 'string' },
          severity: { type: 'string' },
          evidence: {
            type: 'array',
            items: { type: 'string' },
          },
          paths: {
            type: 'array',
            items: { type: 'string' },
          },
          confidence_0_1: {
            type: 'number',
            minimum: 0,
            maximum: 1,
          },
        },
      },
    },
    next_steps: {
      type: 'array',
      minItems: 1,
      items: { type: 'string' },
    },
    confidence_0_1: {
      type: 'number',
      minimum: 0,
      maximum: 1,
    },
    warnings: {
      type: 'array',
      items: { type: 'string' },
    },
    notes: {
      type: 'array',
      items: { type: 'string' },
    },
  },
};

const DEAD_PRIVATE_OUTPUT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['task_kind', 'repo_name', 'summary', 'candidate_clusters', 'confidence_0_1'],
  properties: {
    task_kind: { const: 'dead_private' },
    repo_name: { type: 'string', minLength: 1 },
    summary: { type: 'string', minLength: 1 },
    candidate_clusters: {
      type: 'array',
      minItems: 1,
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['file_path', 'summary', 'evidence', 'confidence_0_1'],
        properties: {
          file_path: { type: 'string', minLength: 1 },
          symbol: { type: 'string' },
          kind: { type: 'string' },
          summary: { type: 'string', minLength: 1 },
          evidence: {
            type: 'array',
            minItems: 1,
            items: { type: 'string' },
          },
          rationale: { type: 'string' },
          confidence_0_1: {
            type: 'number',
            minimum: 0,
            maximum: 1,
          },
          lines: {
            type: 'array',
            items: { type: 'integer' },
          },
        },
      },
    },
    confidence_0_1: {
      type: 'number',
      minimum: 0,
      maximum: 1,
    },
    warnings: {
      type: 'array',
      items: { type: 'string' },
    },
  },
};

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

function buildOutputSchema(task) {
  if (task.kind === 'agent_brief') {
    return AGENT_BRIEF_OUTPUT_SCHEMA;
  }

  return DEAD_PRIVATE_OUTPUT_SCHEMA;
}

function buildTaskPrompt(scenario, scenarioPath, task) {
  const repoRoot = resolveRepoRoot(scenario, scenarioPath);
  const lines = [
    `Repository: ${scenario.repo.name}`,
    `Repository root: ${repoRoot}`,
    `Task kind: ${task.kind}`,
  ];

  if (task.kind === 'agent_brief') {
    lines.push(`Mode: ${task.mode}`);
  }

  lines.push('');
  lines.push(task.prompt.trim());
  return lines.join('\n');
}

function defaultChecksForTask(task) {
  if (task.kind === 'agent_brief') {
    return [
      { kind: 'has', path: 'task_kind', severity: 'required' },
      { kind: 'enum', path: 'mode', allowed: [task.mode], severity: 'required' },
      { kind: 'has', path: 'summary', severity: 'required' },
      { kind: 'min_items', path: 'top_signals', min: 1, severity: 'required' },
      { kind: 'min_items', path: 'next_steps', min: 1, severity: 'required' },
      { kind: 'has', path: 'confidence_0_1', severity: 'required' },
    ];
  }

  return [
    { kind: 'has', path: 'task_kind', severity: 'required' },
    { kind: 'has', path: 'summary', severity: 'required' },
    { kind: 'min_items', path: 'candidate_clusters', min: 1, severity: 'required' },
    { kind: 'has', path: 'confidence_0_1', severity: 'required' },
  ];
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
      result.observed_summary = Array.isArray(observed)
        ? `array(len=${observed.length})`
        : summarizeValue(observed);
      break;
    }
    case 'max_items': {
      result.passed = Array.isArray(observed) && observed.length <= Number(check.max ?? 0);
      result.message = result.passed ? `length <= ${check.max}` : `length > ${check.max}`;
      result.observed_summary = Array.isArray(observed)
        ? `array(len=${observed.length})`
        : summarizeValue(observed);
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
    default:
      fail(`Unsupported check kind: ${check.kind}`);
  }

  if (!result.expected_summary && Object.prototype.hasOwnProperty.call(check, 'value')) {
    result.expected_summary = summarizeValue(check.value);
  }

  return result;
}

function evaluateTask(task, responseJson, providerStatus) {
  const checks =
    Array.isArray(task.checks) && task.checks.length > 0 ? task.checks : defaultChecksForTask(task);
  const checkResults = checks.map((check) => runCheck(responseJson, check));
  const requiredChecks = checkResults.filter((check) => check.severity !== 'optional');
  const requiredFailures = requiredChecks.filter((check) => !check.passed);
  const optionalFailures = checkResults.filter((check) => check.severity === 'optional' && !check.passed);
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
    summary:
      status === 'pass'
        ? 'all required checks passed'
        : status === 'warn'
          ? 'required checks passed, optional checks failed'
          : 'provider or required checks failed',
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
      (task.kind === 'dead_private' && directJson.task_kind === 'dead_private'));

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

async function runProvider(options) {
  if (options.provider === 'claude-code') {
    return runClaudeCode({
      cwd: options.cwd,
      prompt: options.prompt,
      model: options.model,
      jsonSchema: options.jsonSchema,
      appendSystemPrompt: options.appendSystemPrompt,
      timeoutMs: options.timeoutMs,
      claudeBin: options.claudeBin,
    });
  }

  if (options.provider === 'codex-cli') {
    return runCodexExec({
      cwd: options.cwd,
      prompt: options.prompt,
      model: options.model,
      jsonSchema: options.jsonSchema,
      timeoutMs: options.timeoutMs,
      codexBin: options.codexBin,
    });
  }

  fail(`Unsupported provider: ${options.provider}`);
}

function buildDryRunProviderOutput(options, repoRoot) {
  const executable = options.provider === 'codex-cli' ? options.codexBin : options.claudeBin;

  return {
    provider: options.provider,
    provider_version: null,
    command: {
      executable,
      args: [],
    },
    cwd: repoRoot,
    started_at: nowIso(),
    duration_ms: 0,
    exit_code: 0,
    signal: null,
    timed_out: false,
    stdout: '',
    stderr: '',
    stdout_json: null,
    stdout_jsonl: [],
    last_message: null,
  };
}

export async function runEvalTask({ scenario, scenarioPath, task, options, finishedAt }) {
  const repoRoot = resolveRepoRoot(scenario, scenarioPath);
  assertScenarioRepoExists(repoRoot);

  const providerOutput = options.dryRun
    ? buildDryRunProviderOutput(options, repoRoot)
    : await runProvider({
        provider: options.provider,
        cwd: repoRoot,
        prompt: buildTaskPrompt(scenario, scenarioPath, task),
        model: options.model,
        jsonSchema: buildOutputSchema(task),
        appendSystemPrompt: BASE_APPEND_SYSTEM_PROMPT,
        timeoutMs: options.timeoutMs,
        claudeBin: options.claudeBin,
        codexBin: options.codexBin,
      });

  const responsePayload = extractResponsePayload(providerOutput, task);
  const evaluation = options.dryRun
    ? {
        status: 'dry_run',
        score_0_100: 0,
        required_check_count: 0,
        passed_check_count: 0,
        failed_check_count: 0,
        check_results: [],
        provider_failed: false,
        summary: 'dry run skipped provider execution',
      }
    : evaluateTask(task, responsePayload.response_json, providerOutput);

  return {
    schema_version: 1,
    generated_at: finishedAt,
    run_id: options.runId,
    scenario: {
      source_path: scenarioPath,
      ...buildScenarioSummary(scenario),
    },
    task: {
      task_id: task.task_id,
      kind: task.kind,
      mode: task.mode ?? null,
      prompt: task.prompt,
      notes: task.notes ?? null,
      checks: task.checks ?? [],
    },
    provider: {
      name: providerOutput.provider,
      version: providerOutput.provider_version,
      model: options.model ?? null,
      command: providerOutput.command,
      cwd: providerOutput.cwd,
      timeout_ms: options.timeoutMs,
    },
    execution: {
      started_at: providerOutput.started_at,
      finished_at: finishedAt,
      duration_ms: providerOutput.duration_ms,
      exit_code: providerOutput.exit_code,
      signal: providerOutput.signal,
      timed_out: providerOutput.timed_out,
    },
    response: {
      parse_status: responsePayload.parse_status,
      outer_json: responsePayload.outer_json,
      response_json: responsePayload.response_json,
      response_text: responsePayload.response_text,
      stdout: providerOutput.stdout,
      stderr: providerOutput.stderr,
    },
    evaluation,
  };
}

export function buildTaskResultSummary(item, resultPath, result) {
  return {
    scenario_id: item.scenario.scenario_id,
    task_id: item.task.task_id,
    kind: item.task.kind,
    mode: item.task.mode ?? null,
    result_path: resultPath,
    status: result.evaluation.status,
    score_0_100: result.evaluation.score_0_100,
  };
}

export function summarizeTaskResults(taskResults) {
  return {
    task_count: taskResults.length,
    pass_count: taskResults.filter((task) => task.status === 'pass').length,
    warn_count: taskResults.filter((task) => task.status === 'warn').length,
    fail_count: taskResults.filter((task) => task.status === 'fail').length,
    dry_run_count: taskResults.filter((task) => task.status === 'dry_run').length,
  };
}

export function buildRunIndex({
  runId,
  options,
  scenarios,
  taskResults,
  startedAt,
  durationMs,
  buildRunScenarioEntry,
}) {
  const finishedAt = nowIso();
  return {
    schema_version: 1,
    generated_at: finishedAt,
    run_id: runId,
    provider: options.provider,
    model: options.model ?? null,
    dry_run: false,
    output_dir: options.outputDir,
    started_at: startedAt,
    finished_at: finishedAt,
    duration_ms: durationMs,
    scenarios: scenarios.map(buildRunScenarioEntry),
    tasks: taskResults,
    summary: summarizeTaskResults(taskResults),
  };
}
