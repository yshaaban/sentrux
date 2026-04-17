import { readFile } from 'node:fs/promises';
import path from 'node:path';

import {
  defaultOutputDir as buildDefaultOutputDir,
  parseCliArgs,
} from '../../lib/eval-support.mjs';

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
    timeoutMs: Number(process.env.EVAL_TIMEOUT_MS ?? '1800000'),
    idleTimeoutMs: Number(process.env.EVAL_IDLE_TIMEOUT_MS ?? '600000'),
    pollMs: Number(process.env.EVAL_POLL_MS ?? '4000'),
    outputDir: null,
    keepClone: false,
    codexBin: process.env.CODEX_BIN ?? 'codex',
  };

  parseCliArgs(argv, result, {
    flags: {
      '--keep-clone': function enableKeepClone(target) {
        target.keepClone = true;
      },
    },
    values: {
      '--source-root': function setSourceRoot(target, value) {
        target.sourceRoot = value;
      },
      '--repo-label': function setRepoLabel(target, value) {
        target.repoLabel = value;
      },
      '--task': function setTask(target, value) {
        target.task = value;
      },
      '--task-id': function setTaskId(target, value) {
        target.taskId = value;
      },
      '--task-file': function setTaskFile(target, value) {
        target.taskFile = value;
      },
      '--task-label': function setTaskLabel(target, value) {
        target.taskLabel = value;
      },
      '--tag': function appendTag(target, value) {
        target.tags.push(value);
      },
      '--expected-signal-kind': function appendExpectedSignalKind(target, value) {
        target.expectedSignalKinds.push(value);
      },
      '--expected-fix-surface': function setExpectedFixSurface(target, value) {
        target.expectedFixSurface = value;
      },
      '--rules-source': function setRulesSource(target, value) {
        target.rulesSource = value;
      },
      '--analysis-mode': function setAnalysisMode(target, value) {
        target.analysisMode = value;
      },
      '--model': function setModel(target, value) {
        target.model = value;
      },
      '--timeout-ms': function setTimeoutMs(target, value) {
        target.timeoutMs = Number(value);
      },
      '--idle-timeout-ms': function setIdleTimeoutMs(target, value) {
        target.idleTimeoutMs = Number(value);
      },
      '--poll-ms': function setPollMs(target, value) {
        target.pollMs = Number(value);
      },
      '--output-dir': function setOutputDir(target, value) {
        target.outputDir = value;
      },
      '--codex-bin': function setCodexBin(target, value) {
        target.codexBin = value;
      },
    },
  });

  if (!result.task && !result.taskFile) {
    throw new Error('Provide either --task or --task-file');
  }
  if (!Number.isFinite(result.timeoutMs) || result.timeoutMs <= 0) {
    throw new Error(`Invalid --timeout-ms value: ${result.timeoutMs}`);
  }
  if (!Number.isFinite(result.idleTimeoutMs) || result.idleTimeoutMs < 0) {
    throw new Error(`Invalid --idle-timeout-ms value: ${result.idleTimeoutMs}`);
  }
  if (!Number.isFinite(result.pollMs) || result.pollMs <= 0) {
    throw new Error(`Invalid --poll-ms value: ${result.pollMs}`);
  }

  return result;
}

export async function loadPrompt(args) {
  if (args.task) {
    return args.task;
  }

  return readFile(args.taskFile, 'utf8');
}

export function defaultOutputDir(sourceRoot, taskLabel) {
  return buildDefaultOutputDir(sourceRoot, 'task', taskLabel);
}

export function resolveRepoLabel(sourceRoot, repoLabel) {
  return repoLabel ?? path.basename(sourceRoot);
}
