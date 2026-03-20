#!/usr/bin/env node

import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runClaudeCode } from './providers/claude-code.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const defaultManifestPath = path.join(repoRoot, 'docs/v2/evals/index.json');
const defaultOutputDir = path.join(
  repoRoot,
  'docs/v2/evals/runs',
  new Date().toISOString().replace(/[:.]/g, '-'),
);

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

function nowIso() {
  return new Date().toISOString();
}

function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
}

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const result = {
    manifestPath: defaultManifestPath,
    scenarioPaths: [],
    outputDir: defaultOutputDir,
    provider: process.env.EVAL_PROVIDER ?? 'claude-code',
    model: process.env.EVAL_MODEL ?? null,
    timeoutMs: Number(process.env.EVAL_TIMEOUT_MS ?? '1800000'),
    concurrency: Number(process.env.EVAL_CONCURRENCY ?? '1'),
    claudeBin: process.env.CLAUDE_BIN ?? 'claude',
    dryRun: false,
    help: false,
  };

  for (let i = 2; i < argv.length; i += 1) {
    const value = argv[i];
    if (value === '--help' || value === '-h') {
      result.help = true;
      continue;
    }
    if (value === '--dry-run') {
      result.dryRun = true;
      continue;
    }
    if (value === '--manifest') {
      i += 1;
      result.manifestPath = argv[i];
      continue;
    }
    if (value === '--scenario') {
      i += 1;
      result.scenarioPaths.push(argv[i]);
      continue;
    }
    if (value === '--output-dir') {
      i += 1;
      result.outputDir = argv[i];
      continue;
    }
    if (value === '--provider') {
      i += 1;
      result.provider = argv[i];
      continue;
    }
    if (value === '--model') {
      i += 1;
      result.model = argv[i];
      continue;
    }
    if (value === '--timeout-ms') {
      i += 1;
      result.timeoutMs = Number(argv[i]);
      continue;
    }
    if (value === '--concurrency') {
      i += 1;
      result.concurrency = Number(argv[i]);
      continue;
    }
    if (value === '--claude-bin') {
      i += 1;
      result.claudeBin = argv[i];
      continue;
    }
    fail(`Unknown argument: ${value}`);
  }

  if (!Number.isFinite(result.timeoutMs) || result.timeoutMs <= 0) {
    fail(`Invalid --timeout-ms value: ${result.timeoutMs}`);
  }

  if (!Number.isFinite(result.concurrency) || result.concurrency < 1) {
    fail(`Invalid --concurrency value: ${result.concurrency}`);
  }

  return result;
}

function readJson(filePath) {
  return readFile(filePath, 'utf8').then((text) => JSON.parse(text));
}

function writeJson(filePath, value) {
  return mkdir(path.dirname(filePath), { recursive: true }).then(() =>
    writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8'),
  );
}

function resolvePath(basePath, candidatePath) {
  if (path.isAbsolute(candidatePath)) {
    return candidatePath;
  }

  return path.resolve(basePath, candidatePath);
}

function ensureScenarioShape(scenario, scenarioPath) {
  if (!scenario || typeof scenario !== 'object') {
    fail(`Scenario is not an object: ${scenarioPath}`);
  }

  if (scenario.schema_version !== 1) {
    fail(`Unsupported scenario schema version in ${scenarioPath}: ${scenario.schema_version}`);
  }

  if (typeof scenario.scenario_id !== 'string' || !scenario.scenario_id) {
    fail(`Scenario missing scenario_id: ${scenarioPath}`);
  }

  if (!scenario.repo || typeof scenario.repo !== 'object') {
    fail(`Scenario missing repo block: ${scenarioPath}`);
  }

  if (typeof scenario.repo.name !== 'string' || !scenario.repo.name) {
    fail(`Scenario missing repo.name: ${scenarioPath}`);
  }

  if (typeof scenario.repo.root_env !== 'string' || !scenario.repo.root_env) {
    fail(`Scenario missing repo.root_env: ${scenarioPath}`);
  }

  if (typeof scenario.repo.default_root !== 'string' || !scenario.repo.default_root) {
    fail(`Scenario missing repo.default_root: ${scenarioPath}`);
  }

  if (!Array.isArray(scenario.tasks) || scenario.tasks.length === 0) {
    fail(`Scenario has no tasks: ${scenarioPath}`);
  }

  for (const task of scenario.tasks) {
    if (!task || typeof task !== 'object') {
      fail(`Scenario task is not an object in ${scenarioPath}`);
    }
    if (typeof task.task_id !== 'string' || !task.task_id) {
      fail(`Scenario task missing task_id in ${scenarioPath}`);
    }
    if (task.kind !== 'agent_brief' && task.kind !== 'dead_private') {
      fail(`Scenario task has unsupported kind "${task.kind}" in ${scenarioPath}`);
    }
    if (typeof task.prompt !== 'string' || !task.prompt.trim()) {
      fail(`Scenario task missing prompt in ${scenarioPath}:${task.task_id}`);
    }
    if (task.kind === 'agent_brief') {
      if (task.mode !== 'repo_onboarding' && task.mode !== 'patch' && task.mode !== 'pre_merge') {
        fail(`agent_brief task has unsupported mode in ${scenarioPath}:${task.task_id}`);
      }
    } else if ('mode' in task) {
      fail(`dead_private task must not define mode in ${scenarioPath}:${task.task_id}`);
    }
    if (task.checks !== undefined && !Array.isArray(task.checks)) {
      fail(`Scenario task checks must be an array in ${scenarioPath}:${task.task_id}`);
    }
  }
}

function loadScenarioPathsFromManifest(manifestPath) {
  const manifestDir = path.dirname(manifestPath);
  return readJson(manifestPath).then((manifest) => {
    if (!manifest || typeof manifest !== 'object') {
      fail(`Manifest is not an object: ${manifestPath}`);
    }

    if (!Array.isArray(manifest.scenarios) || manifest.scenarios.length === 0) {
      fail(`Manifest has no scenarios: ${manifestPath}`);
    }

    return manifest.scenarios.map((entry) => {
      if (!entry || typeof entry !== 'object') {
        fail(`Invalid scenario entry in manifest: ${manifestPath}`);
      }

      if (typeof entry.path !== 'string' || !entry.path) {
        fail(`Scenario entry missing path in manifest: ${manifestPath}`);
      }

      return resolvePath(manifestDir, entry.path);
    });
  });
}

function resolveRepoRoot(scenario) {
  const override = process.env[scenario.repo.root_env];
  return override ? path.resolve(override) : path.resolve(scenario.repo.default_root);
}

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

function buildTaskPrompt(scenario, task) {
  const lines = [
    `Repository: ${scenario.repo.name}`,
    `Repository root: ${resolveRepoRoot(scenario)}`,
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
      result.message = result.passed
        ? `length >= ${check.min}`
        : `length < ${check.min}`;
      result.observed_summary = Array.isArray(observed)
        ? `array(len=${observed.length})`
        : summarizeValue(observed);
      break;
    }
    case 'max_items': {
      result.passed = Array.isArray(observed) && observed.length <= Number(check.max ?? 0);
      result.message = result.passed
        ? `length <= ${check.max}`
        : `length > ${check.max}`;
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
  const checks = Array.isArray(task.checks) && task.checks.length > 0 ? task.checks : defaultChecksForTask(task);
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

function buildScenarioSummary(scenario) {
  return {
    scenario_id: scenario.scenario_id,
    title: scenario.title ?? null,
    description: scenario.description ?? null,
    repo: scenario.repo,
  };
}

function buildResultPath(outputDir, scenario, task) {
  return path.join(outputDir, scenario.scenario_id, task.task_id, 'result.json');
}

async function runTask({ scenario, scenarioPath, task, outputDir, options }) {
  const repoRoot = resolveRepoRoot(scenario);
  if (!existsSync(repoRoot)) {
    fail(`Scenario repo root does not exist: ${repoRoot}`);
  }

  const providerOutput = options.dryRun
    ? {
        provider: options.provider,
        provider_version: null,
        command: { executable: options.claudeBin, args: [] },
        cwd: repoRoot,
        started_at: nowIso(),
        duration_ms: 0,
        exit_code: 0,
        signal: null,
        timed_out: false,
        stdout: '',
        stderr: '',
        stdout_json: null,
      }
    : await runClaudeCode({
        cwd: repoRoot,
        prompt: buildTaskPrompt(scenario, task),
        model: options.model,
        jsonSchema: buildOutputSchema(task),
        appendSystemPrompt: BASE_APPEND_SYSTEM_PROMPT,
        timeoutMs: options.timeoutMs,
        claudeBin: options.claudeBin,
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

  const finishedAt = nowIso();
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

async function runWithConcurrency(items, concurrency, worker) {
  const results = new Array(items.length);
  let nextIndex = 0;

  const workers = Array.from({ length: Math.min(concurrency, items.length) }, async () => {
    while (true) {
      const index = nextIndex;
      nextIndex += 1;
      if (index >= items.length) {
        return;
      }
      results[index] = await worker(items[index], index);
    }
  });

  await Promise.all(workers);
  return results;
}

function printHelp() {
  console.log(`Usage: node scripts/evals/run.mjs [options]

Options:
  --manifest <path>     Load scenario file list from a manifest JSON file
  --scenario <path>     Run a single scenario file; repeatable
  --output-dir <path>   Write results to this directory
  --provider <name>     Provider name (default: claude-code)
  --model <name>        Claude model alias or full name
  --timeout-ms <n>      Provider timeout in milliseconds
  --concurrency <n>    Number of tasks to run in parallel
  --claude-bin <path>   Path to the Claude Code CLI binary
  --dry-run             Validate scenarios without calling any provider
  --help                Show this help text
`);
}

async function main() {
  const options = parseArgs(process.argv);
  if (options.help) {
    printHelp();
    return;
  }

  if (options.provider !== 'claude-code') {
    fail(`Unsupported provider: ${options.provider}`);
  }

  const runId = `eval-${new Date().toISOString().replace(/[:.]/g, '-')}`;
  options.runId = runId;

  const scenarioPaths =
    options.scenarioPaths.length > 0
      ? options.scenarioPaths.map((scenarioPath) => path.resolve(scenarioPath))
      : await loadScenarioPathsFromManifest(path.resolve(options.manifestPath));

  const scenarios = [];
  for (const scenarioPath of scenarioPaths) {
    const scenario = await readJson(scenarioPath);
    ensureScenarioShape(scenario, scenarioPath);
    scenarios.push({ scenario, scenarioPath });
  }

  await mkdir(options.outputDir, { recursive: true });

  if (options.dryRun) {
    const dryRunIndex = {
      schema_version: 1,
      generated_at: nowIso(),
      run_id: runId,
      provider: options.provider,
      dry_run: true,
      output_dir: options.outputDir,
      scenarios: scenarios.map(({ scenario, scenarioPath }) => ({
        scenario_id: scenario.scenario_id,
        source_path: scenarioPath,
        repo: scenario.repo,
        task_count: scenario.tasks.length,
        tasks: scenario.tasks.map((task) => ({
          task_id: task.task_id,
          kind: task.kind,
          mode: task.mode ?? null,
        })),
      })),
    };

    await writeJson(path.join(options.outputDir, 'index.json'), dryRunIndex);
    console.log(
      `Dry run loaded ${scenarios.length} scenario(s) and ${dryRunIndex.scenarios.reduce(
        (count, scenario) => count + scenario.task_count,
        0,
      )} task(s).`,
    );
    return;
  }

  const allTasks = [];
  for (const entry of scenarios) {
    for (const task of entry.scenario.tasks) {
      allTasks.push({
        scenario: entry.scenario,
        scenarioPath: entry.scenarioPath,
        task,
      });
    }
  }

  const startedAt = nowIso();
  const startedMs = nowMs();
  const taskResults = await runWithConcurrency(allTasks, options.concurrency, async (item) => {
    const result = await runTask({
      scenario: item.scenario,
      scenarioPath: item.scenarioPath,
      task: item.task,
      outputDir: options.outputDir,
      options,
    });

    const resultPath = buildResultPath(options.outputDir, item.scenario, item.task);
    await writeJson(resultPath, result);
    return {
      scenario_id: item.scenario.scenario_id,
      task_id: item.task.task_id,
      kind: item.task.kind,
      mode: item.task.mode ?? null,
      result_path: resultPath,
      status: result.evaluation.status,
      score_0_100: result.evaluation.score_0_100,
    };
  });

  const finishedAt = nowIso();
  const index = {
    schema_version: 1,
    generated_at: finishedAt,
    run_id: runId,
    provider: options.provider,
    model: options.model ?? null,
    dry_run: false,
    output_dir: options.outputDir,
    started_at: startedAt,
    finished_at: finishedAt,
    duration_ms: Number((nowMs() - startedMs).toFixed(1)),
    scenarios: scenarios.map(({ scenario, scenarioPath }) => ({
      scenario_id: scenario.scenario_id,
      source_path: scenarioPath,
      repo: scenario.repo,
      task_count: scenario.tasks.length,
    })),
    tasks: taskResults,
    summary: {
      task_count: taskResults.length,
      pass_count: taskResults.filter((task) => task.status === 'pass').length,
      warn_count: taskResults.filter((task) => task.status === 'warn').length,
      fail_count: taskResults.filter((task) => task.status === 'fail').length,
    },
  };

  await writeJson(path.join(options.outputDir, 'index.json'), index);
  console.log(
    `Completed ${index.summary.task_count} task(s): ${index.summary.pass_count} pass, ${index.summary.warn_count} warn, ${index.summary.fail_count} fail.`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : String(error));
  process.exitCode = 1;
});
