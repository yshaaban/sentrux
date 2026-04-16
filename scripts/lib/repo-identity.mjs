import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, readdirSync, readFileSync, readlinkSync, statSync } from 'node:fs';
import { cp, mkdir, rm } from 'node:fs/promises';
import path from 'node:path';

function runGit(repoRoot, args) {
  const result = spawnSync('git', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    shell: false,
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`git ${args.join(' ')} exited with code ${result.status}`);
  }

  return result.stdout ?? '';
}

function splitNullDelimited(value) {
  return value.split('\0').map((entry) => entry.trimEnd()).filter(Boolean);
}

function shouldTrackPathForIdentity(relPath) {
  return !relPath.startsWith('.sentrux/');
}

function parsePorcelainStatusRecords(value) {
  const records = splitNullDelimited(value);
  const dirtyRecords = [];

  for (let index = 0; index < records.length; index += 1) {
    const entry = records[index];
    const status = entry.slice(0, 2);
    const firstPath = entry.slice(3);

    if (status[0] === 'R' || status[0] === 'C') {
      const secondPath = records[index + 1];
      dirtyRecords.push({
        status,
        paths: [firstPath, secondPath].filter(Boolean),
      });
      if (secondPath) {
        index += 1;
      }
      continue;
    }

    dirtyRecords.push({
      status,
      paths: [firstPath].filter(Boolean),
    });
  }

  return dirtyRecords;
}

function hashFileContents(targetPath) {
  return createHash('sha256').update(readFileSync(targetPath)).digest('hex');
}

function hashSymlinkTarget(targetPath) {
  return createHash('sha256').update(readlinkSync(targetPath)).digest('hex');
}

function updateDirectoryHash(hash, rootPath, currentPath) {
  const entries = readdirSync(currentPath, { withFileTypes: true }).sort(function compareEntries(left, right) {
    return left.name.localeCompare(right.name);
  });

  for (const entry of entries) {
    if (entry.name === '.git') {
      continue;
    }

    const entryPath = path.join(currentPath, entry.name);
    const relPath = path.relative(rootPath, entryPath).split(path.sep).join('/');

    hash.update(relPath);
    hash.update('\0');
    if (entry.isSymbolicLink()) {
      hash.update('symlink');
      hash.update('\0');
      hash.update(hashSymlinkTarget(entryPath));
      hash.update('\0');
      continue;
    }
    if (entry.isDirectory()) {
      hash.update('directory');
      hash.update('\0');
      updateDirectoryHash(hash, rootPath, entryPath);
      continue;
    }

    hash.update('file');
    hash.update('\0');
    hash.update(hashFileContents(entryPath));
    hash.update('\0');
  }
}

function hashDirectoryContents(targetPath) {
  const hash = createHash('sha256');
  hash.update('directory-contents-fingerprint-v1\0');
  updateDirectoryHash(hash, targetPath, targetPath);
  return hash.digest('hex');
}

function hashPathIdentity(targetPath) {
  const stat = lstatSync(targetPath);
  if (stat.isSymbolicLink()) {
    return `symlink:${hashSymlinkTarget(targetPath)}`;
  }
  if (stat.isDirectory()) {
    return `directory:${hashDirectoryContents(targetPath)}`;
  }

  return `file:${hashFileContents(targetPath)}`;
}

function fingerprintPathList(repoRoot, relPaths) {
  const hash = createHash('sha256');
  hash.update('repo-tree-fingerprint-v1\0');

  for (const relPath of relPaths) {
    const normalizedPath = relPath.split(path.sep).join('/');
    const absPath = path.join(repoRoot, relPath);
    hash.update(normalizedPath);
    hash.update('\0');

    if (existsSync(absPath)) {
      hash.update(hashPathIdentity(absPath));
    } else {
      hash.update('deleted');
    }

    hash.update('\0');
  }

  return hash.digest('hex');
}

function dirtyPathsForRepo(repoRoot) {
  const dirtyRecords = parsePorcelainStatusRecords(
    runGit(repoRoot, ['status', '--porcelain=v1', '-z', '--untracked-files=all']),
  );
  return dirtyRecords
    .flatMap((record) => record.paths)
    .filter(Boolean)
    .filter(shouldTrackPathForIdentity);
}

export function collectFileIdentity(targetPath) {
  if (!targetPath) {
    return null;
  }

  const exists = existsSync(targetPath);
  if (exists && statSync(targetPath).isDirectory()) {
    return {
      path: targetPath,
      exists: true,
      sha256: null,
    };
  }

  return {
    path: targetPath,
    exists,
    sha256: exists ? hashFileContents(targetPath) : null,
  };
}

export function collectRepoIdentity(repoRoot) {
  const commit = runGit(repoRoot, ['rev-parse', 'HEAD']).trim();
  const dirtyPaths = dirtyPathsForRepo(repoRoot);
  const treePaths = splitNullDelimited(
    runGit(repoRoot, ['ls-files', '-z', '--cached', '--others', '--exclude-standard']),
  )
    .filter(shouldTrackPathForIdentity)
    .sort();

  return {
    commit,
    dirty_paths: dirtyPaths,
    dirty_paths_count: dirtyPaths.length,
    dirty_paths_fingerprint: fingerprintPathList(repoRoot, [...dirtyPaths].sort()),
    tree_fingerprint: fingerprintPathList(repoRoot, treePaths),
  };
}

export function resolveHeadCommitEpoch(repoRoot) {
  const stdout = runGit(repoRoot, ['log', '-1', '--format=%ct']).trim();
  const epoch = Number.parseInt(stdout, 10);
  if (!Number.isInteger(epoch) || epoch < 0) {
    throw new Error(`Could not resolve HEAD commit epoch for ${repoRoot}`);
  }
  return epoch;
}

export async function overlayWorkingTreeChanges({ sourceRoot, targetRoot }) {
  const dirtyPaths = [...new Set(dirtyPathsForRepo(sourceRoot))].sort();

  for (const relPath of dirtyPaths) {
    const sourcePath = path.join(sourceRoot, relPath);
    const targetPath = path.join(targetRoot, relPath);

    if (existsSync(sourcePath)) {
      await mkdir(path.dirname(targetPath), { recursive: true });
      await cp(sourcePath, targetPath, { force: true, recursive: true });
      continue;
    }

    await rm(targetPath, { recursive: true, force: true });
  }
}

export function buildRepoFreshnessMetadata({
  repoRoot,
  analyzedRoot = repoRoot,
  analysisMode,
  rulesSource = null,
  binaryPath = null,
}) {
  const sourceTreeIdentity = collectRepoIdentity(repoRoot);
  const analyzedTreeIdentity = collectRepoIdentity(analyzedRoot);

  return {
    analysis_mode: analysisMode,
    source_tree_identity: {
      ...sourceTreeIdentity,
      analysis_mode: analysisMode,
    },
    analyzed_tree_identity: {
      ...analyzedTreeIdentity,
      analysis_mode: analysisMode,
    },
    rules_identity: collectFileIdentity(rulesSource),
    binary_identity: collectFileIdentity(binaryPath),
    repo_root: repoRoot,
  };
}

export function compareRepoIdentity(expected, actual) {
  const mismatches = [];

  for (const key of ['commit', 'dirty_paths_count', 'dirty_paths_fingerprint', 'tree_fingerprint']) {
    if (expected?.[key] !== actual?.[key]) {
      mismatches.push({
        key,
        expected: expected?.[key] ?? null,
        actual: actual?.[key] ?? null,
      });
    }
  }

  const expectedPaths = JSON.stringify(expected?.dirty_paths ?? []);
  const actualPaths = JSON.stringify(actual?.dirty_paths ?? []);
  if (expectedPaths !== actualPaths) {
    mismatches.push({
      key: 'dirty_paths',
      expected: expected?.dirty_paths ?? [],
      actual: actual?.dirty_paths ?? [],
    });
  }

  if (expected?.analysis_mode !== actual?.analysis_mode) {
    mismatches.push({
      key: 'analysis_mode',
      expected: expected?.analysis_mode ?? null,
      actual: actual?.analysis_mode ?? null,
    });
  }

  return mismatches;
}

export function compareFileIdentity(expected, actual, label) {
  const mismatches = [];

  for (const key of ['exists', 'sha256']) {
    if (expected?.[key] !== actual?.[key]) {
      mismatches.push({
        key: `${label}.${key}`,
        expected: expected?.[key] ?? null,
        actual: actual?.[key] ?? null,
      });
    }
  }

  return mismatches;
}

export function assertRepoIdentityFresh({
  expected,
  actual,
  label = 'repo identity',
  allowStale = false,
}) {
  const mismatches = compareRepoIdentity(expected, actual);
  if (mismatches.length === 0) {
    return;
  }

  const summary = mismatches
    .map((mismatch) => {
      const expectedValue =
        typeof mismatch.expected === 'string' ? `"${mismatch.expected}"` : JSON.stringify(mismatch.expected);
      const actualValue =
        typeof mismatch.actual === 'string' ? `"${mismatch.actual}"` : JSON.stringify(mismatch.actual);
      return `${mismatch.key}: expected ${expectedValue}, got ${actualValue}`;
    })
    .join('; ');

  if (allowStale) {
    console.warn(`Stale ${label} accepted: ${summary}`);
    return;
  }

  throw new Error(`Stale ${label}: ${summary}`);
}

export function assertFileIdentityFresh({
  expected,
  actual,
  label = 'file identity',
  allowStale = false,
}) {
  const mismatches = compareFileIdentity(expected, actual, label);
  if (mismatches.length === 0) {
    return;
  }

  const summary = mismatches
    .map((mismatch) => {
      const expectedValue =
        typeof mismatch.expected === 'string' ? `"${mismatch.expected}"` : JSON.stringify(mismatch.expected);
      const actualValue =
        typeof mismatch.actual === 'string' ? `"${mismatch.actual}"` : JSON.stringify(mismatch.actual);
      return `${mismatch.key}: expected ${expectedValue}, got ${actualValue}`;
    })
    .join('; ');

  if (allowStale) {
    console.warn(`Stale ${label} accepted: ${summary}`);
    return;
  }

  throw new Error(`Stale ${label}: ${summary}`);
}
