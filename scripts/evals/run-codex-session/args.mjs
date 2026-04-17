import { readFile } from 'node:fs/promises';

import {
  parseCliArgs,
} from '../../lib/eval-support.mjs';
import {
  appendStringOption,
  assertNonNegativeNumberOption,
  assertPositiveNumberOption,
  defaultEvalIdleTimeoutMs,
  defaultEvalOutputDir as buildDefaultEvalOutputDir,
  defaultEvalPollMs,
  defaultEvalTimeoutMs,
  resolveRepoLabel as buildRepoLabel,
  setFlag,
  setNumberOption,
  setStringOption,
} from '../../lib/eval-cli-shared.mjs';

export function parseArgs(argv) {
  const result = {
    sourceRoot: process.cwd(),
    repoLabel: null,
    taskId: null,
    task: null,
    taskFile: null,
    taskLabel: null,
    tags: [],
    expectedSignalKinds: [],
    expectedFixSurface: null,
    rulesSource: null,
    analysisMode: 'working_tree',
    model: process.env.EVAL_MODEL ?? null,
    timeoutMs: defaultEvalTimeoutMs(),
    idleTimeoutMs: defaultEvalIdleTimeoutMs(),
    pollMs: defaultEvalPollMs(),
    outputDir: null,
    keepClone: false,
    codexBin: process.env.CODEX_BIN ?? 'codex',
  };

  parseCliArgs(argv, result, {
    flags: {
      '--keep-clone': setFlag('keepClone'),
    },
    values: {
      '--source-root': setStringOption('sourceRoot'),
      '--repo-label': setStringOption('repoLabel'),
      '--task': setStringOption('task'),
      '--task-id': setStringOption('taskId'),
      '--task-file': setStringOption('taskFile'),
      '--task-label': setStringOption('taskLabel'),
      '--tag': appendStringOption('tags'),
      '--expected-signal-kind': appendStringOption('expectedSignalKinds'),
      '--expected-fix-surface': setStringOption('expectedFixSurface'),
      '--rules-source': setStringOption('rulesSource'),
      '--analysis-mode': setStringOption('analysisMode'),
      '--model': setStringOption('model'),
      '--timeout-ms': setNumberOption('timeoutMs'),
      '--idle-timeout-ms': setNumberOption('idleTimeoutMs'),
      '--poll-ms': setNumberOption('pollMs'),
      '--output-dir': setStringOption('outputDir'),
      '--codex-bin': setStringOption('codexBin'),
    },
  });

  if (!result.task && !result.taskFile) {
    throw new Error('Provide either --task or --task-file');
  }
  assertPositiveNumberOption('--timeout-ms', result.timeoutMs);
  assertNonNegativeNumberOption('--idle-timeout-ms', result.idleTimeoutMs);
  assertPositiveNumberOption('--poll-ms', result.pollMs);

  return result;
}

export async function loadPrompt(args) {
  if (args.task) {
    return args.task;
  }

  return readFile(args.taskFile, 'utf8');
}

export function defaultOutputDir(sourceRoot, taskLabel) {
  return buildDefaultEvalOutputDir(sourceRoot, 'task', taskLabel);
}

export function resolveRepoLabel(sourceRoot, repoLabel) {
  return buildRepoLabel(sourceRoot, repoLabel);
}
