import { spawn } from 'node:child_process';

import { nowMs } from './shared.mjs';
import { createStdoutEventTracker } from './stdout-events.mjs';

export function spawnCaptured(command, args, options) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ['ignore', 'pipe', 'pipe'],
      shell: false,
    });

    let stdout = '';
    let stderr = '';
    let timedOut = false;
    const timeout =
      options.timeoutMs && options.timeoutMs > 0
        ? setTimeout(() => {
            timedOut = true;
            child.kill('SIGKILL');
          }, options.timeoutMs)
        : null;

    child.stdout.on('data', (chunk) => {
      const text = chunk.toString('utf8');
      stdout += text;
    });
    child.stderr.on('data', (chunk) => {
      const text = chunk.toString('utf8');
      stderr += text;
    });
    child.on('error', (error) => {
      if (timeout) {
        clearTimeout(timeout);
      }
      reject(error);
    });
    child.on('close', (exitCode, signal) => {
      if (timeout) {
        clearTimeout(timeout);
      }
      resolve({
        exitCode,
        signal,
        stdout,
        stderr,
        timedOut,
      });
    });
  });
}

export function startCaptured(command, args, options) {
  const child = spawn(command, args, {
    cwd: options.cwd,
    env: options.env,
    stdio: ['ignore', 'pipe', 'pipe'],
    shell: false,
  });

  let stdout = '';
  let stderr = '';
  let stdoutLength = 0;
  let stderrLength = 0;
  let timedOut = false;
  let finished = false;
  let lastOutputAtMs = nowMs();
  const eventTracker = createStdoutEventTracker();
  const timeout =
    options.timeoutMs && options.timeoutMs > 0
      ? setTimeout(() => {
          timedOut = true;
          child.kill('SIGKILL');
        }, options.timeoutMs)
      : null;

  const resultPromise = new Promise((resolve, reject) => {
    child.stdout.on('data', (chunk) => {
      const text = chunk.toString('utf8');
      stdout += text;
      stdoutLength += text.length;
      lastOutputAtMs = nowMs();
      eventTracker.consume(text);
    });
    child.stderr.on('data', (chunk) => {
      const text = chunk.toString('utf8');
      stderr += text;
      stderrLength += text.length;
      lastOutputAtMs = nowMs();
    });
    child.on('error', (error) => {
      finished = true;
      if (timeout) {
        clearTimeout(timeout);
      }
      reject(error);
    });
    child.on('close', (exitCode, signal) => {
      finished = true;
      if (timeout) {
        clearTimeout(timeout);
      }
      eventTracker.finish();
      resolve({
        exitCode,
        signal,
        stdout,
        stderr,
        timedOut,
        eventSummary: eventTracker.snapshot(),
      });
    });
  });

  return {
    child,
    get finished() {
      return finished;
    },
    get stdoutLength() {
      return stdoutLength;
    },
    get stderrLength() {
      return stderrLength;
    },
    get lastOutputAtMs() {
      return lastOutputAtMs;
    },
    get eventSummary() {
      return eventTracker.snapshot();
    },
    kill(signal = 'SIGKILL') {
      if (!finished) {
        child.kill(signal);
      }
    },
    wait() {
      return resultPromise;
    },
  };
}
