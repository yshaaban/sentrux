#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import process from 'node:process';

const COMMANDS = [
  ['cargo', ['fmt', '--all', '--check']],
  ['cargo', ['test', '-p', 'sentrux-core', '--', '--nocapture']],
  ['cargo', ['build', '-p', 'sentrux']],
  ['cargo', ['build', '--release', '-p', 'sentrux']],
  ['npm', ['--prefix', 'ts-bridge', 'test']],
  ['node', ['scripts/validate_parallel_code_v2.mjs', '--goldens-only']],
  ['node', ['scripts/benchmark_sentrux_v2.mjs']],
  ['node', ['scripts/benchmark_parallel_code_v2.mjs']],
  ['git', ['diff', '--check']],
  ['node', ['scripts/check_public_release_hygiene.mjs']],
];

function resolveInstallArtifactName() {
  switch (process.platform) {
    case 'darwin':
      if (process.arch === 'arm64') {
        return 'sentrux-darwin-arm64';
      }
      return null;
    case 'linux':
      if (process.arch === 'x64') {
        return 'sentrux-linux-x86_64';
      }
      if (process.arch === 'arm64') {
        return 'sentrux-linux-aarch64';
      }
      return null;
    default:
      return null;
  }
}

function runCommand(command, args) {
  console.log(`\n$ ${command} ${args.join(' ')}`);
  const result = spawnSync(command, args, {
    cwd: process.cwd(),
    stdio: 'inherit',
    env: process.env,
  });

  if (result.status !== 0) {
    throw new Error(`${command} exited with status ${result.status ?? 'unknown'}`);
  }
}

function runInstallSmokeIfSupported() {
  const artifactName = resolveInstallArtifactName();
  if (!artifactName) {
    console.log(
      `\n# Skipping install smoke: unsupported local platform ${process.platform}/${process.arch}`,
    );
    return;
  }

  runCommand('./scripts/smoke_test_install.sh', [
    '--artifact-path',
    'target/release/sentrux',
    '--artifact-name',
    artifactName,
  ]);
}

function main() {
  for (const [command, args] of COMMANDS) {
    runCommand(command, args);
  }

  runInstallSmokeIfSupported();
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
