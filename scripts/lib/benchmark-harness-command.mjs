import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { cp, rename, rm } from 'node:fs/promises';
import { nowMs, roundMs } from './benchmark-harness-metrics.mjs';

export async function runCommand(command, args, options = {}) {
  const {
    cwd = process.cwd(),
    env = {},
    homeOverride = null,
    input = null,
    skipGrammarDownload = null,
  } = options;
  const startedAt = nowMs();
  const child = spawn(command, args, {
    cwd,
    env: {
      ...process.env,
      ...env,
      ...(homeOverride ? { HOME: homeOverride } : {}),
      ...(skipGrammarDownload ? { SENTRUX_SKIP_GRAMMAR_DOWNLOAD: skipGrammarDownload } : {}),
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  let stdout = '';
  let stderr = '';
  child.stdout.setEncoding('utf8');
  child.stderr.setEncoding('utf8');
  child.stdout.on('data', function appendStdout(chunk) {
    stdout += chunk;
  });
  child.stderr.on('data', function appendStderr(chunk) {
    stderr += chunk;
  });

  const exit = new Promise(function waitForExit(resolve, reject) {
    child.once('error', reject);
    child.once('close', function onClose(exitCode, signal) {
      resolve({
        exit_code: exitCode ?? null,
        signal: signal ?? null,
      });
    });
  });

  if (input !== null) {
    child.stdin.end(input);
  } else {
    child.stdin.end();
  }

  const result = await exit;
  return {
    elapsed_ms: roundMs(nowMs() - startedAt),
    exit_code: result.exit_code,
    signal: result.signal,
    stdout,
    stderr,
  };
}

export async function runBenchmarkCommand(command, label, args, summarize, options = {}) {
  const result = await runCommand(command, args, options);

  return {
    label,
    command,
    args,
    elapsed_ms: result.elapsed_ms,
    summary: summarize(result),
    exit_code: result.exit_code,
    signal: result.signal,
  };
}

export async function backupFileIfExists(targetPath, backupPath) {
  if (!existsSync(targetPath)) {
    return false;
  }

  await cp(targetPath, backupPath);
  return true;
}

export async function restoreManagedFile(targetPath, backupPath, existedBefore) {
  if (existedBefore) {
    await rename(backupPath, targetPath);
    return;
  }

  if (existsSync(targetPath)) {
    await rm(targetPath, { force: true });
  }
}
