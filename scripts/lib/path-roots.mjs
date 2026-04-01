import { existsSync } from 'node:fs';
import path from 'node:path';

export function resolveWorkspaceRepoRoot(envValue, fallbackRepoName, repoRoot) {
  if (typeof envValue === 'string' && envValue.trim()) {
    return envValue;
  }

  return path.resolve(repoRoot, '..', fallbackRepoName);
}

export function assertRepoRootExists(repoRootPath, label) {
  if (!existsSync(repoRootPath)) {
    throw new Error(`Missing ${label}: ${repoRootPath}`);
  }
}
