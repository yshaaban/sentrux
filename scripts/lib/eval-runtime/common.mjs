import { execFile as execFileCallback } from 'node:child_process';
import { access, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';

const execFile = promisify(execFileCallback);

export function nowIso() {
  return new Date().toISOString();
}

export function nowMs() {
  return Number(process.hrtime.bigint()) / 1_000_000;
}

export function fail(message) {
  throw new Error(message);
}

export async function pathExists(targetPath) {
  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}

export async function readJson(filePath) {
  const text = await readFile(filePath, 'utf8');
  return JSON.parse(text);
}

export async function writeJson(filePath, value) {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

export function resolvePath(basePath, candidatePath) {
  if (path.isAbsolute(candidatePath)) {
    return candidatePath;
  }

  return path.resolve(basePath, candidatePath);
}

export async function runNodeScript(scriptPath, args, { cwd, env = {} } = {}) {
  const { stdout, stderr } = await execFile(process.execPath, [scriptPath, ...args], {
    cwd,
    maxBuffer: 1024 * 1024 * 20,
    env: {
      ...process.env,
      ...env,
    },
  });

  return {
    stdout: stdout.trim(),
    stderr: stderr.trim(),
  };
}

export async function runGit(repoRootPath, gitArgs) {
  try {
    const { stdout } = await execFile('git', gitArgs, {
      cwd: repoRootPath,
      maxBuffer: 1024 * 1024,
    });

    return stdout.trim();
  } catch {
    return null;
  }
}

export async function collectRepoMetadata(repoRootPath) {
  const branch = await runGit(repoRootPath, ['rev-parse', '--abbrev-ref', 'HEAD']);
  const commit = await runGit(repoRootPath, ['rev-parse', '--short', 'HEAD']);
  const status = await runGit(repoRootPath, ['status', '--short']);

  return {
    branch,
    commit,
    workingTreeClean: status === '',
  };
}

export async function runWithConcurrency(items, concurrency, worker) {
  const results = new Array(items.length);
  let nextIndex = 0;
  const workerCount = Math.min(concurrency, items.length);
  const workers = Array.from({ length: workerCount }, async function runWorker() {
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
