#!/usr/bin/env node

import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { parseCliArgs } from '../lib/eval-support.mjs';
import {
  appendStringOption,
  assertAtLeastOneNumberOption,
  assertPositiveNumberOption,
  defaultDocsEvalRunOutputDir,
  defaultEvalTimeoutMs,
  setFlag,
  setNumberOption,
  setStringOption,
} from '../lib/eval-cli-shared.mjs';
import {
  fail,
  nowIso,
  nowMs,
  runWithConcurrency,
  writeJson,
} from '../lib/eval-runtime/common.mjs';
import {
  buildDryRunScenarioEntry,
  buildResultPath,
  buildRunScenarioEntry,
  buildScenarioTaskQueue,
  countScenarioTasks,
  loadScenarioEntries,
} from '../lib/eval-runtime/scenarios.mjs';
import {
  buildRunIndex,
  buildTaskResultSummary,
  runEvalTask,
} from '../lib/eval-runtime/provider-task-runner.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const defaultManifestPath = path.join(repoRoot, 'docs/v2/evals/index.json');
const defaultOutputDir = defaultDocsEvalRunOutputDir(repoRoot);

function parseArgs(argv) {
  const result = {
    manifestPath: defaultManifestPath,
    scenarioPaths: [],
    outputDir: defaultOutputDir,
    provider: process.env.EVAL_PROVIDER ?? 'claude-code',
    model: process.env.EVAL_MODEL ?? null,
    timeoutMs: defaultEvalTimeoutMs(),
    concurrency: Number(process.env.EVAL_CONCURRENCY ?? '1'),
    claudeBin: process.env.CLAUDE_BIN ?? 'claude',
    codexBin: process.env.CODEX_BIN ?? 'codex',
    dryRun: false,
    help: false,
  };

  parseCliArgs(argv, result, {
    flags: {
      '--help': setFlag('help'),
      '-h': setFlag('help'),
      '--dry-run': setFlag('dryRun'),
    },
    values: {
      '--manifest': setStringOption('manifestPath'),
      '--scenario': appendStringOption('scenarioPaths'),
      '--output-dir': setStringOption('outputDir'),
      '--provider': setStringOption('provider'),
      '--model': setStringOption('model'),
      '--timeout-ms': setNumberOption('timeoutMs'),
      '--concurrency': setNumberOption('concurrency'),
      '--claude-bin': setStringOption('claudeBin'),
      '--codex-bin': setStringOption('codexBin'),
    },
  });

  try {
    assertPositiveNumberOption('--timeout-ms', result.timeoutMs);
    assertAtLeastOneNumberOption('--concurrency', result.concurrency);
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
  }

  return result;
}

function printHelp() {
  console.log(`Usage: node scripts/evals/run.mjs [options]

Options:
  --manifest <path>     Load scenario file list from a manifest JSON file
  --scenario <path>     Run a single scenario file; repeatable
  --output-dir <path>   Write results to this directory
  --provider <name>     Provider name (default: claude-code; supports claude-code, codex-cli, minimax-openai)
  --model <name>        Provider model name
  --timeout-ms <n>      Provider timeout in milliseconds
  --concurrency <n>    Number of tasks to run in parallel
  --claude-bin <path>   Path to the Claude Code CLI binary
  --codex-bin <path>    Path to the Codex CLI binary
  --dry-run             Validate scenarios without calling any provider
  --help                Show this help text

MiniMax auth:
  Set MINIMAX_API_KEY (or OPENAI_API_KEY) for --provider minimax-openai.
  Optionally set MINIMAX_BASE_URL to override the default https://api.minimax.io/v1 endpoint.
`);
}

async function writeDryRunIndex(options, runId, scenarios) {
  const dryRunIndex = {
    schema_version: 1,
    generated_at: nowIso(),
    run_id: runId,
    provider: options.provider,
    dry_run: true,
    output_dir: options.outputDir,
    scenarios: scenarios.map(buildDryRunScenarioEntry),
  };

  await writeJson(path.join(options.outputDir, 'index.json'), dryRunIndex);
  console.log(
    `Dry run loaded ${scenarios.length} scenario(s) and ${countScenarioTasks(dryRunIndex.scenarios)} task(s).`,
  );
}

async function runScenarioTasks(scenarios, options) {
  const allTasks = buildScenarioTaskQueue(scenarios);
  return runWithConcurrency(allTasks, options.concurrency, async (item) => {
    const result = await runEvalTask({
      scenario: item.scenario,
      scenarioPath: item.scenarioPath,
      task: item.task,
      options,
      finishedAt: nowIso(),
    });

    const resultPath = buildResultPath(options.outputDir, item.scenario, item.task);
    await writeJson(resultPath, result);
    return buildTaskResultSummary(item, resultPath, result);
  });
}

async function main() {
  const options = parseArgs(process.argv);
  if (options.help) {
    printHelp();
    return;
  }

  const runId = `eval-${new Date().toISOString().replace(/[:.]/g, '-')}`;
  options.runId = runId;
  const scenarios = await loadScenarioEntries(options);
  await mkdir(options.outputDir, { recursive: true });

  if (options.dryRun) {
    await writeDryRunIndex(options, runId, scenarios);
    return;
  }

  const startedAt = nowIso();
  const startedMs = nowMs();
  const taskResults = await runScenarioTasks(scenarios, options);
  const index = buildRunIndex({
    runId,
    options,
    scenarios,
    taskResults,
    startedAt,
    durationMs: Number((nowMs() - startedMs).toFixed(1)),
    buildRunScenarioEntry,
  });
  await writeJson(path.join(options.outputDir, 'index.json'), index);
  console.log(
    `Completed ${index.summary.task_count} task(s): ${index.summary.pass_count} pass, ${index.summary.warn_count} warn, ${index.summary.fail_count} fail.`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
