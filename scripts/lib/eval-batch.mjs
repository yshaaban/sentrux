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

export function normalizeExecutionOutcome(outcome, executionStatus = 'completed') {
  const isCompletedExecution = !executionStatus || executionStatus === 'completed';
  const normalizedOutcome = {
    session_count: outcome?.session_count ?? null,
    final_gate: outcome?.final_gate ?? null,
    final_session_clean: outcome?.final_session_clean ?? false,
    initial_top_action_kind: outcome?.initial_top_action_kind ?? null,
    initial_action_kinds: outcome?.initial_action_kinds ?? [],
    top_action_cleared: outcome?.top_action_cleared ?? false,
    checks_to_clear_top_action: outcome?.checks_to_clear_top_action ?? null,
    convergence_status: outcome?.convergence_status ?? null,
    entropy_delta: outcome?.entropy_delta ?? null,
    followup_regression_introduced:
      outcome?.followup_regression_introduced ?? false,
  };

  if (isCompletedExecution) {
    return normalizedOutcome;
  }

  normalizedOutcome.final_session_clean = false;
  normalizedOutcome.top_action_cleared = false;
  normalizedOutcome.checks_to_clear_top_action = null;
  if (!normalizedOutcome.final_gate || normalizedOutcome.final_gate === 'pass') {
    normalizedOutcome.final_gate = 'warn';
  }

  if (executionStatus === 'provider_failed') {
    normalizedOutcome.convergence_status = 'provider_failed';
    return normalizedOutcome;
  }

  if (
    !normalizedOutcome.convergence_status ||
    normalizedOutcome.convergence_status === 'converged'
  ) {
    normalizedOutcome.convergence_status = 'stalled';
  }

  return normalizedOutcome;
}

export function summarizeBundleOutcome(bundle) {
  const initialActions =
    bundle?.outcome?.initial_action_kinds ??
    bundle?.initial_check?.actions?.map((action) => action.kind).filter(Boolean) ??
    [];
  const sessionCount =
    bundle?.telemetry_summary?.summary?.session_count ?? bundle?.outcome?.session_count ?? null;
  const outcome = {
    session_count: sessionCount,
    final_gate: bundle?.outcome?.final_gate ?? null,
    final_session_clean: bundle?.outcome?.final_session_clean ?? false,
    initial_top_action_kind: bundle?.outcome?.initial_top_action_kind ?? null,
    initial_action_kinds: initialActions,
    top_action_cleared: bundle?.outcome?.top_action_cleared ?? false,
    checks_to_clear_top_action: bundle?.outcome?.checks_to_clear_top_action ?? null,
    convergence_status: bundle?.outcome?.convergence_status ?? null,
    entropy_delta: bundle?.outcome?.entropy_delta ?? null,
    followup_regression_introduced:
      bundle?.outcome?.followup_regression_introduced ?? false,
  };

  return normalizeExecutionOutcome(outcome, bundle?.status ?? 'completed');
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
