import path from 'node:path';

import { defaultOutputDir as buildDefaultOutputDir } from './eval-support.mjs';

export function defaultEvalTimeoutMs() {
  return Number(process.env.EVAL_TIMEOUT_MS ?? '1800000');
}

export function defaultEvalIdleTimeoutMs() {
  return Number(process.env.EVAL_IDLE_TIMEOUT_MS ?? '600000');
}

export function defaultEvalPollMs() {
  return Number(process.env.EVAL_POLL_MS ?? '4000');
}

export function defaultDocsEvalRunOutputDir(repoRoot) {
  return path.join(
    repoRoot,
    'docs/v2/evals/runs',
    new Date().toISOString().replace(/[:.]/g, '-'),
  );
}

export function defaultEvalOutputDir(sourceRoot, prefix, label) {
  return buildDefaultOutputDir(sourceRoot, prefix, label);
}

export function resolveRepoLabel(sourceRoot, repoLabel) {
  return repoLabel ?? path.basename(sourceRoot);
}

export function setFlag(property) {
  return function setFlagValue(target) {
    target[property] = true;
  };
}

export function setStringOption(property) {
  return function setStringOptionValue(target, value) {
    target[property] = value;
  };
}

export function setNumberOption(property) {
  return function setNumberOptionValue(target, value) {
    target[property] = Number(value);
  };
}

export function appendStringOption(property) {
  return function appendStringOptionValue(target, value) {
    target[property].push(value);
  };
}

export function assertPositiveNumberOption(option, value) {
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`Invalid ${option} value: ${value}`);
  }
}

export function assertNonNegativeNumberOption(option, value) {
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`Invalid ${option} value: ${value}`);
  }
}

export function assertAtLeastOneNumberOption(option, value) {
  if (!Number.isFinite(value) || value < 1) {
    throw new Error(`Invalid ${option} value: ${value}`);
  }
}
