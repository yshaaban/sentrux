import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { collectFileIdentity, collectRepoIdentity } from '../lib/repo-identity.mjs';

function runGit(repoRoot, args) {
  execFileSync('git', ['-C', repoRoot, ...args], {
    stdio: 'pipe',
  });
}

test('collectRepoIdentity hashes untracked directory entries without throwing', async function () {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), 'sentrux-repo-identity-test-'));

  try {
    runGit(repoRoot, ['init', '-q']);
    runGit(repoRoot, ['config', 'user.email', 'test@example.com']);
    runGit(repoRoot, ['config', 'user.name', 'Sentrux Test']);

    await writeFile(path.join(repoRoot, 'tracked.txt'), 'tracked\n');
    runGit(repoRoot, ['add', 'tracked.txt']);
    runGit(repoRoot, ['commit', '-q', '-m', 'init']);

    const worktreeDir = path.join(repoRoot, '.worktrees', 'task', 'demo');
    await mkdir(worktreeDir, { recursive: true });
    runGit(worktreeDir, ['init', '-q']);
    runGit(worktreeDir, ['config', 'user.email', 'nested@example.com']);
    runGit(worktreeDir, ['config', 'user.name', 'Nested Test']);
    await writeFile(path.join(worktreeDir, 'nested.txt'), 'one\n');
    runGit(worktreeDir, ['add', 'nested.txt']);
    runGit(worktreeDir, ['commit', '-q', '-m', 'nested init']);

    const beforeEdit = collectRepoIdentity(repoRoot);
    assert(beforeEdit.dirty_paths.includes('.worktrees/task/demo/'));
    assert.match(beforeEdit.dirty_paths_fingerprint, /^[0-9a-f]{64}$/);
    assert.match(beforeEdit.tree_fingerprint, /^[0-9a-f]{64}$/);

    await writeFile(path.join(worktreeDir, 'nested.txt'), 'two\n');
    const afterEdit = collectRepoIdentity(repoRoot);
    assert.notEqual(afterEdit.dirty_paths_fingerprint, beforeEdit.dirty_paths_fingerprint);
  } finally {
    await rm(repoRoot, { recursive: true, force: true });
  }
});

test('collectFileIdentity leaves directory paths unhashed', async function () {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), 'sentrux-file-identity-test-'));

  try {
    const directoryPath = path.join(tempRoot, 'dir');
    await mkdir(directoryPath, { recursive: true });

    assert.deepEqual(collectFileIdentity(directoryPath), {
      path: directoryPath,
      exists: true,
      sha256: null,
    });
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});
