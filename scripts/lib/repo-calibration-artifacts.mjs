import { access, readFile } from 'node:fs/promises';
import path from 'node:path';

export async function pathExists(targetPath) {
  if (!targetPath) {
    return false;
  }

  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}

export async function readJson(targetPath) {
  const source = await readFile(targetPath, 'utf8');
  return JSON.parse(source);
}

export function defaultLatestCalibrationPath(repoRootPath, repoLabel) {
  return path.join(repoRootPath, '.sentrux', 'evals', repoLabel, 'latest.json');
}

export async function resolveLatestRepoCalibrationArtifacts({
  repoRootPath,
  repoLabel,
  latestCalibrationPath = null,
}) {
  if (!repoRootPath || !repoLabel) {
    return null;
  }

  const pointerPath = path.resolve(
    latestCalibrationPath ?? defaultLatestCalibrationPath(repoRootPath, repoLabel),
  );
  if (!(await pathExists(pointerPath))) {
    return null;
  }

  const latestPointer = await readJson(pointerPath);
  const summaryPath = path.resolve(
    latestPointer.summary_json ??
      path.join(latestPointer.latest_output_dir ?? '', 'repo-calibration-loop.json'),
  );
  if (!(await pathExists(summaryPath))) {
    return null;
  }

  const summary = await readJson(summaryPath);

  return {
    pointerPath,
    summaryPath,
    latestPointer,
    summary,
    artifacts: summary.artifacts ?? {},
    cohortId: summary.cohort_id ?? null,
  };
}
