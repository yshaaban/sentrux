import { readJson } from '../eval-batch.mjs';

export async function loadRepoCalibrationManifest(manifestPath) {
  const manifest = await readJson(manifestPath);
  if (manifest?.schema_version !== 1) {
    throw new Error(`Unsupported repo calibration manifest: ${manifestPath}`);
  }

  return manifest;
}
