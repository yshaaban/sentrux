import { copyFile, mkdir, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { runNodeScript as runSharedNodeScript } from '../eval-runtime/common.mjs';

export async function acquireLoopLock(lockPath, metadata) {
  await mkdir(path.dirname(lockPath), { recursive: true });

  try {
    await mkdir(lockPath);
  } catch (error) {
    if (error && typeof error === 'object' && error.code === 'EEXIST') {
      throw new Error(`Another calibration loop already holds the repo lock: ${lockPath}`);
    }
    throw error;
  }

  await writeFile(
    path.join(lockPath, 'owner.json'),
    `${JSON.stringify(metadata, null, 2)}\n`,
    'utf8',
  );
  return async function releaseLoopLock() {
    await rm(lockPath, { recursive: true, force: true });
  };
}

async function publishArtifact(sourcePath, targetPath) {
  if (!sourcePath || !targetPath) {
    return;
  }

  await mkdir(path.dirname(targetPath), { recursive: true });
  const tempPath = `${targetPath}.tmp-${process.pid}-${Date.now()}`;
  await copyFile(sourcePath, tempPath);
  await rename(tempPath, targetPath);
}

export async function publishArtifacts(pairs) {
  for (const pair of pairs) {
    if (!pair?.sourcePath || !pair?.targetPath) {
      continue;
    }

    await publishArtifact(pair.sourcePath, pair.targetPath);
  }
}

export function buildBatchRunArgs(
  manifestPath,
  outputDir,
  cohortManifestPath,
  cohortId,
) {
  const args = ['--manifest', manifestPath, '--output-dir', outputDir];

  if (cohortManifestPath) {
    args.push('--cohort-manifest', cohortManifestPath);
  }
  if (cohortId) {
    args.push('--cohort-id', cohortId);
  }

  return args;
}

export async function runNodeScript(repoRoot, scriptPath, args) {
  const result = await runSharedNodeScript(scriptPath, args, { cwd: repoRoot });

  return {
    script: path.relative(repoRoot, scriptPath),
    args,
    stdout: result.stdout,
    stderr: result.stderr,
  };
}
