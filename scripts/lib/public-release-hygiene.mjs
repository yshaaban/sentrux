import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

export const PUBLIC_HYGIENE_RULES = [
  {
    id: 'abandoned_public_repo',
    pattern: /github\.com\/sentrux\/sentrux\b|(?<!github\.com\/)sentrux\/sentrux\b/g,
    message: 'Replace abandoned sentrux/sentrux links with the canonical public repo.',
  },
  {
    id: 'private_release_dependency',
    pattern: /\bprivate-integration-crate\b|\bPRO_REPO_TOKEN\b|\bHOMEBREW_TAP_TOKEN\b/g,
    message: 'Public release paths must not depend on private private-integration-crate or hidden release secrets.',
  },
  {
    id: 'internal_repo_name',
    pattern: /\bprivate-benchmark-repo-a\b|\bprivate-benchmark-repo-b\b|\bprivate-feedback-fixture\b|\bforge\b/g,
    message: 'Checked-in public files must not mention removed internal benchmark repos.',
  },
  {
    id: 'internal_env_var',
    pattern: /\bPRIVATE_BENCHMARK_ROOT_A\b|\bPRIVATE_BENCHMARK_ROOT_B\b/g,
    message: 'Checked-in public files must not mention removed internal benchmark env vars.',
  },
  {
    id: 'internal_domain',
    pattern: /gitlab\.humain\.com/g,
    message: 'Checked-in public files must not point at internal GitLab infrastructure.',
  },
  {
    id: 'workstation_path',
    pattern: /\/home\/yrsh\//g,
    message: 'Checked-in public files must not embed maintainer workstation paths.',
  },
  {
    id: 'forge_fixture_label',
    pattern: /\/workspace\/private-repo\b|repoLabel["']?\s*[:=]\s*["']private-repo["']/g,
    message: 'Public fixtures should use generic external-repo labels, not private-repo-specific ones.',
  },
];

export const PUBLIC_HYGIENE_IGNORED_PATHS = new Set([
  'scripts/lib/public-release-hygiene.mjs',
  'scripts/tests/public-release-hygiene.test.mjs',
]);

export function listRepoFiles(rootDir) {
  const output = execFileSync(
    'git',
    ['ls-files', '-z', '--cached', '--others', '--exclude-standard'],
    {
      cwd: rootDir,
      encoding: 'utf8',
    },
  );

  return output
    .split('\0')
    .map(function trimEntry(entry) {
      return entry.trim();
    })
    .filter(Boolean);
}

export function scanText(text, filePath) {
  const matches = [];

  for (const rule of PUBLIC_HYGIENE_RULES) {
    for (const match of text.matchAll(rule.pattern)) {
      const before = text.slice(0, match.index);
      const line = before.split('\n').length;
      matches.push({
        filePath,
        line,
        ruleId: rule.id,
        message: rule.message,
        snippet: match[0],
      });
    }
  }

  return matches;
}

export function isBinaryContent(buffer) {
  return buffer.includes(0);
}

function isMissingFileError(error) {
  return Boolean(error && typeof error === 'object' && error.code === 'ENOENT');
}

export async function scanFiles(rootDir, filePaths) {
  const matches = [];

  for (const relativePath of filePaths) {
    if (PUBLIC_HYGIENE_IGNORED_PATHS.has(relativePath)) {
      continue;
    }

    const absolutePath = path.join(rootDir, relativePath);
    let buffer;
    try {
      buffer = await readFile(absolutePath);
    } catch (error) {
      if (isMissingFileError(error)) {
        continue;
      }
      throw error;
    }

    if (isBinaryContent(buffer)) {
      continue;
    }

    const text = buffer.toString('utf8');
    matches.push(...scanText(text, relativePath));
  }

  return matches;
}
