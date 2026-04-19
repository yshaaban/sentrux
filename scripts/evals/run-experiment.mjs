#!/usr/bin/env node

import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  appendStringOption,
  setFlag,
} from '../lib/eval-cli-shared.mjs';
import { writeJson, writeText } from '../lib/eval-batch.mjs';
import { parseCliArgs } from '../lib/eval-support.mjs';
import {
  buildExperimentRunPlan,
  executeExperimentPlan,
  formatExperimentRunMarkdown,
} from '../lib/experiment-program.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');

function setResolvedPathOption(property) {
  return function setResolvedPathValue(target, value) {
    target[property] = path.resolve(value);
  };
}

export function parseArgs(argv) {
  return parseCliArgs(
    argv,
    {
      experimentPath: null,
      outputJson: null,
      outputMarkdown: null,
      repoIds: [],
      runIds: [],
      variantIds: [],
      dryRun: false,
      continueOnError: false,
    },
    {
      flags: {
        '--dry-run': setFlag('dryRun'),
        '--continue-on-error': setFlag('continueOnError'),
      },
      values: {
        '--experiment': setResolvedPathOption('experimentPath'),
        '--output-json': setResolvedPathOption('outputJson'),
        '--output-md': setResolvedPathOption('outputMarkdown'),
        '--repo-id': appendStringOption('repoIds'),
        '--run-id': appendStringOption('runIds'),
        '--variant-id': appendStringOption('variantIds'),
      },
    },
  );
}

function buildDryRunResult(plan) {
  return {
    schema_version: 1,
    generated_at: plan.generated_at,
    experiment_id: plan.spec.experiment_id,
    spec_path: plan.spec_path,
    status: 'dry_run',
    run_results: plan.runs.map(function buildDryRunRunResult(run) {
      return {
        run_id: run.run_id,
        repo_id: run.repo_id,
        variant_id: run.variant_id,
        execution_mode: run.execution_mode,
        status: 'planned',
        command: run.command,
        output_dir: run.output_dir,
        policy_override_path: run.policy_override_path ?? null,
      };
    }),
  };
}

async function buildRunResult(plan, args) {
  if (args.dryRun) {
    return buildDryRunResult(plan);
  }

  return executeExperimentPlan(plan, {
    continueOnError: args.continueOnError,
  });
}

async function main(argv = process.argv) {
  const args = parseArgs(argv);
  if (!args.experimentPath) {
    throw new Error('Missing required argument: --experiment <path>');
  }

  const plan = await buildExperimentRunPlan({
    specPath: args.experimentPath,
    repoRootPath: repoRoot,
    repoIds: args.repoIds,
    runIds: args.runIds,
    variantIds: args.variantIds,
  });
  const result = await buildRunResult(plan, args);
  const markdown = formatExperimentRunMarkdown(result, plan);

  if (args.outputJson) {
    await writeJson(args.outputJson, result);
  }
  if (args.outputMarkdown) {
    await writeText(args.outputMarkdown, markdown);
  }

  process.stdout.write(markdown);
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;

if (invokedPath === import.meta.url) {
  main().catch(function handleMainError(error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
