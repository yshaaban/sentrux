import { cp, mkdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { createMcpSession } from './benchmark-harness.mjs';

export function slugifyEvalLabel(value) {
  return String(value ?? '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'eval';
}

export function defaultRulesSource(sourceRoot) {
  const candidate = path.join(sourceRoot, '.sentrux', 'rules.toml');
  return existsSync(candidate) ? candidate : null;
}

export function defaultOutputDir(sourceRoot, prefix, label) {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  return path.join(sourceRoot, '.sentrux', 'evals', `${timestamp}-${prefix}-${slugifyEvalLabel(label)}`);
}

function requireOptionValue(argv, index, option) {
  const valueIndex = index + 1;
  const value = argv[valueIndex];
  if (value === undefined) {
    throw new Error(`Missing value for ${option}`);
  }

  return {
    nextIndex: valueIndex,
    value,
  };
}

export function parseCliArgs(argv, result, { flags = {}, values = {} }) {
  for (let index = 2; index < argv.length; index += 1) {
    const option = argv[index];
    const handleFlag = flags[option];
    if (handleFlag) {
      handleFlag(result);
      continue;
    }

    const handleValue = values[option];
    if (handleValue) {
      const { nextIndex, value } = requireOptionValue(argv, index, option);
      index = nextIndex;
      handleValue(result, value);
      continue;
    }

    throw new Error(`Unknown argument: ${option}`);
  }

  return result;
}

export function createEvalMcpSession({
  repoRoot,
  binPath,
  homeOverride,
  skipGrammarDownload = process.env.SENTRUX_SKIP_GRAMMAR_DOWNLOAD ?? '1',
  requestTimeoutMs = Number(process.env.REQUEST_TIMEOUT_MS ?? '120000'),
}) {
  return createMcpSession({
    binPath,
    repoRoot,
    homeOverride,
    skipGrammarDownload,
    requestTimeoutMs,
  });
}

export async function maybeCopyFile(sourcePath, targetPath) {
  if (!sourcePath || !existsSync(sourcePath)) {
    return false;
  }

  await mkdir(path.dirname(targetPath), { recursive: true });
  await cp(sourcePath, targetPath);
  return true;
}
