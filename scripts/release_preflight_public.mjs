#!/usr/bin/env node

import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import process from 'node:process';

const TREE_SITTER_CLI_BIN_DIR = path.join(os.homedir(), '.local', 'tree-sitter-cli', 'bin');
const COMMANDS = [
  ['cargo', ['fmt', '--all', '--check']],
  ['npm', ['ci', '--prefix', 'ts-bridge']],
  ['cargo', ['test', '-p', 'sentrux-core', '--', '--nocapture']],
  ['cargo', ['build', '-p', 'sentrux']],
  ['cargo', ['build', '--release', '-p', 'sentrux']],
  ['npm', ['--prefix', 'ts-bridge', 'test']],
  ['node', ['scripts/validate_parallel_code_v2.mjs', '--goldens-only']],
  ['node', ['scripts/check_public_release_hygiene.mjs']],
];

function resolveCurrentPlatformRelease() {
  switch (process.platform) {
    case 'darwin':
      if (process.arch === 'arm64') {
        return {
          artifactName: 'sentrux-darwin-arm64',
          bundlePlatform: 'darwin-arm64',
        };
      }
      return null;
    case 'linux':
      if (process.arch === 'x64') {
        return {
          artifactName: 'sentrux-linux-x86_64',
          bundlePlatform: 'linux-x86_64',
        };
      }
      if (process.arch === 'arm64') {
        return {
          artifactName: 'sentrux-linux-aarch64',
          bundlePlatform: 'linux-aarch64',
        };
      }
      return null;
    default:
      return null;
  }
}

function runCommand(command, args, extraEnv = {}) {
  console.log(`\n$ ${command} ${args.join(' ')}`);
  const result = spawnSync(command, args, {
    cwd: process.cwd(),
    stdio: 'inherit',
    env: {
      ...process.env,
      ...extraEnv,
    },
  });

  if (result.status !== 0) {
    throw new Error(`${command} exited with status ${result.status ?? 'unknown'}`);
  }
}

function ensureCleanTrackedTree(label) {
  function assertGitTreeIsClean(args, description) {
    const result = spawnSync('git', args, {
      cwd: process.cwd(),
      stdio: 'inherit',
    });
    if (result.status !== 0) {
      throw new Error(`${description} is not clean ${label}`);
    }
  }

  assertGitTreeIsClean(['diff', '--quiet', '--ignore-submodules=all'], 'Tracked working tree');
  assertGitTreeIsClean(['diff', '--cached', '--quiet', '--ignore-submodules=all'], 'Index');
}

function treeSitterCliEnv() {
  return {
    PATH: `${TREE_SITTER_CLI_BIN_DIR}:${process.env.PATH ?? ''}`,
  };
}

function ensureTreeSitterCliInstalled() {
  runCommand('bash', ['./scripts/install_tree_sitter_cli.sh']);
}

function runInstallSmokeIfSupported() {
  const release = resolveCurrentPlatformRelease();
  if (!release) {
    console.log(
      `\n# Skipping install smoke: unsupported local platform ${process.platform}/${process.arch}`,
    );
    return;
  }

  const { artifactName, bundlePlatform } = release;
  const tempRoot = mkdtempSync(path.join(os.tmpdir(), 'sentrux-public-preflight-'));
  const grammarBundlePath = path.join(tempRoot, `grammars-${bundlePlatform}.tar.gz`);

  try {
    ensureTreeSitterCliInstalled();
    runCommand('./scripts/build_grammar_bundle.sh', [
      '--platform',
      bundlePlatform,
      '--output',
      grammarBundlePath,
    ], treeSitterCliEnv());
    runCommand('./scripts/smoke_test_install.sh', [
      '--artifact-path',
      'target/release/sentrux',
      '--artifact-name',
      artifactName,
      '--grammar-bundle-path',
      grammarBundlePath,
    ]);
  } finally {
    rmSync(tempRoot, { recursive: true, force: true });
  }
}

function main() {
  ensureCleanTrackedTree('before release preflight');
  for (const [command, args] of COMMANDS) {
    runCommand(command, args);
  }

  runInstallSmokeIfSupported();
  runCommand('git', ['diff', '--check']);
  ensureCleanTrackedTree('after release preflight');
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
