import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { nowIso } from './eval-runtime/common.mjs';
import { writeJson as writeJsonArtifact } from './script-artifacts.mjs';

export { nowIso } from './eval-runtime/common.mjs';

export function slugify(value) {
  return String(value ?? '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'batch';
}

export async function readJson(targetPath) {
  const source = await readFile(targetPath, 'utf8');
  return JSON.parse(source);
}

export { writeJsonArtifact as writeJson };

export async function writeText(targetPath, value) {
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, value, 'utf8');
}

export function defaultBatchOutputDir(sourceRoot, prefix, label) {
  const timestamp = nowIso().replace(/[:.]/g, '-');
  return path.join(sourceRoot, '.sentrux', 'evals', `${timestamp}-${prefix}-${slugify(label)}`);
}

export function parseTagList(value) {
  if (!value) {
    return [];
  }

  if (Array.isArray(value)) {
    return value.filter(Boolean).map(String);
  }

  return String(value)
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean);
}

export function normalizeExpectedSignalKinds(value) {
  if (!value) {
    return [];
  }

  return Array.isArray(value) ? value.filter(Boolean).map(String) : [String(value)];
}

export function summarizeBundleOutcome(bundle) {
  const initialActions =
    bundle?.outcome?.initial_action_kinds ??
    bundle?.initial_check?.actions?.map((action) => action.kind).filter(Boolean) ??
    [];

  return {
    final_gate: bundle?.outcome?.final_gate ?? null,
    final_session_clean: bundle?.outcome?.final_session_clean ?? false,
    initial_top_action_kind: bundle?.outcome?.initial_top_action_kind ?? null,
    initial_action_kinds: initialActions,
    top_action_cleared: bundle?.outcome?.top_action_cleared ?? false,
    checks_to_clear_top_action: bundle?.outcome?.checks_to_clear_top_action ?? null,
    followup_regression_introduced:
      bundle?.outcome?.followup_regression_introduced ?? false,
  };
}

export async function loadBatchManifest(targetPath) {
  const manifest = await readJson(targetPath);
  if (manifest?.schema_version !== 1) {
    throw new Error(`Unsupported batch manifest: ${targetPath}`);
  }

  return manifest;
}

export function resolveManifestPath(manifestPath, relativePath) {
  if (!relativePath) {
    return null;
  }

  if (path.isAbsolute(relativePath)) {
    return relativePath;
  }

  const resolvedManifestPath = path.resolve(manifestPath);
  const manifestDir = path.dirname(resolvedManifestPath);
  return path.resolve(manifestDir, relativePath);
}
