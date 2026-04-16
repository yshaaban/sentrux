import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  PUBLIC_HYGIENE_IGNORED_PATHS,
  PUBLIC_HYGIENE_RULES,
  hashNormalizedToken,
  isBinaryContent,
  listRepoFiles,
  scanFiles,
  scanText,
} from '../lib/public-release-hygiene.mjs';

test('scanText catches abandoned repo links, non-public root vars, and workstation paths', function () {
  const text = [
    'See https://github.com/sentrux/sentrux/releases for the old release page.',
    'Do not publish SECRET_PROJECT_ROOT or /home/tester/private in checked-in artifacts.',
  ].join('\n');

  const matches = scanText(text, 'README.md');
  const ruleIds = matches.map(function toRuleId(match) {
    return match.ruleId;
  });

  assert.ok(ruleIds.includes('abandoned_public_repo'));
  assert.ok(ruleIds.includes('non_public_root_env'));
  assert.ok(ruleIds.includes('workstation_path'));
});

test('scanText catches hashed private-token rules without storing literals in the repo', function () {
  const hashedRule = {
    id: 'synthetic_private_token',
    tokenHashes: new Set([hashNormalizedToken('shadow-repo')]),
    message: 'Synthetic rule for hashed token matching.',
  };
  const matches = scanText('shadow-repo should be redacted from public fixtures.', 'README.md', [
    hashedRule,
  ]);

  assert.deepEqual(
    matches.map(function toRuleId(match) {
      return match.ruleId;
    }),
    ['synthetic_private_token'],
  );
});

test('scanText stays quiet on sanitized public content', function () {
  const text = [
    'Canonical repo: https://github.com/yshaaban/sentrux',
    'Use sentrux mcp for the public MCP server command.',
    'Artifacts must not contain private repo names.',
  ].join('\n');

  assert.deepEqual(scanText(text, 'README.md'), []);
});

test('binary detection skips nul-containing content', function () {
  assert.equal(isBinaryContent(Buffer.from([0x41, 0x00, 0x42])), true);
  assert.equal(isBinaryContent(Buffer.from('plain text')), false);
});

test('rules cover the key public-release leak classes', function () {
  const ruleIds = PUBLIC_HYGIENE_RULES.map(function toRuleId(rule) {
    return rule.id;
  });

  assert.ok(ruleIds.includes('abandoned_public_repo'));
  assert.ok(ruleIds.includes('private_release_dependency'));
  assert.ok(ruleIds.includes('private_repo_token'));
  assert.ok(ruleIds.includes('self_hosted_gitlab'));
  assert.ok(ruleIds.includes('workstation_path'));
});

test('ignored paths cover the hygiene scanner fixtures themselves', function () {
  assert.equal(PUBLIC_HYGIENE_IGNORED_PATHS.has('scripts/lib/public-release-hygiene.mjs'), true);
  assert.equal(
    PUBLIC_HYGIENE_IGNORED_PATHS.has('scripts/tests/public-release-hygiene.test.mjs'),
    true,
  );
});

test('listRepoFiles includes untracked public files and scanFiles reports them', async function () {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), 'sentrux-public-hygiene-'));

  try {
    execFileSync('git', ['init', '-q'], { cwd: repoRoot });
    execFileSync('git', ['config', 'user.email', 'test@example.com'], { cwd: repoRoot });
    execFileSync('git', ['config', 'user.name', 'Sentrux Test'], { cwd: repoRoot });

    await writeFile(path.join(repoRoot, 'tracked.md'), 'clean\n');
    execFileSync('git', ['add', 'tracked.md'], { cwd: repoRoot });
    execFileSync('git', ['commit', '-q', '-m', 'init'], { cwd: repoRoot });

    await writeFile(
      path.join(repoRoot, 'untracked.md'),
      'Old repo link: https://github.com/sentrux/sentrux/releases\n',
    );

    const filePaths = listRepoFiles(repoRoot);
    assert.ok(filePaths.includes('tracked.md'));
    assert.ok(filePaths.includes('untracked.md'));

    const matches = await scanFiles(repoRoot, filePaths);
    assert.ok(matches.some(function hasUntrackedMatch(match) {
      return match.filePath === 'untracked.md' && match.ruleId === 'abandoned_public_repo';
    }));
  } finally {
    await rm(repoRoot, { recursive: true, force: true });
  }
});

test('scanFiles skips deleted tracked paths without failing', async function () {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), 'sentrux-public-hygiene-deleted-'));

  try {
    const matches = await scanFiles(repoRoot, ['missing.md']);
    assert.deepEqual(matches, []);
  } finally {
    await rm(repoRoot, { recursive: true, force: true });
  }
});
