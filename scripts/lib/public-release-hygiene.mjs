import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

const TOKEN_PATTERN = /[A-Za-z0-9._-]+/g;

export function hashNormalizedToken(value) {
  return createHash('sha256').update(value.toLowerCase()).digest('hex');
}

export const PUBLIC_HYGIENE_RULES = [
  {
    id: 'abandoned_public_repo',
    pattern: /github\.com\/sentrux\/sentrux\b|(?<!github\.com\/)sentrux\/sentrux\b/g,
    message: 'Replace abandoned sentrux/sentrux links with the canonical public repo.',
  },
  {
    id: 'private_release_dependency',
    pattern: /\bPRO_REPO_TOKEN\b|\bHOMEBREW_TAP_TOKEN\b/g,
    message: 'Public release paths must not depend on hidden release secrets.',
  },
  {
    id: 'private_repo_token',
    tokenHashes: new Set([
      '08b585e62405dde771dddd9d240c281a839ca8cebe86e1c28b90a25c5915d72e',
      'f2ef853f0fe15e24850789e4b8d867079b7879578ea649933f7753b5a9380d18',
      'ed6b51eb98b53480d60a970adb34b13792dc14bb9073927e554e5565378cb271',
      '521e74f9b998acff86800fe4c86c114af12578f3970c65cd264a3fe4a60521ef',
      '71b41d6dd48dc58eba8f5cf9edf30fef6597fdf285a521bb8fcbad4b3d50887d',
    ]),
    message: 'Checked-in public files must not mention scrubbed private project names.',
  },
  {
    id: 'non_public_root_env',
    pattern: /\b(?!PARALLEL_CODE_ROOT\b)[A-Z][A-Z0-9_]*_ROOT\b/g,
    message: 'Checked-in public files must not introduce non-public repo-root env vars.',
  },
  {
    id: 'self_hosted_gitlab',
    pattern: /\bgitlab\.(?!com\b)[A-Za-z0-9.-]+\b/gi,
    message: 'Checked-in public files must not point at self-hosted GitLab infrastructure.',
  },
  {
    id: 'workstation_path',
    pattern: /\/(?:home|Users)\/[A-Za-z0-9._-]+\//g,
    message: 'Checked-in public files must not embed workstation-specific paths.',
  },
  {
    id: 'non_public_fixture_label',
    pattern:
      /\/workspace\/(?!parallel-code\b|sentrux\b|one-tool\b|external-repo\b|public-repo-feedback\b)[A-Za-z0-9._-]+\b|repo(?:Label|_label)["']?\s*[:=]\s*["'](?!parallel-code\b|sentrux\b|self\b|external-repo\b|public-repo-feedback\b|one-tool\b)[A-Za-z0-9._-]+["']/g,
    message: 'Public fixtures should use only public-safe repo labels.',
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

function buildPatternMatches(rule, text, filePath) {
  const matches = [];

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

  return matches;
}

function buildTokenHashMatches(rule, text, filePath) {
  const matches = [];

  for (const match of text.matchAll(TOKEN_PATTERN)) {
    const tokenHash = hashNormalizedToken(match[0]);
    if (!rule.tokenHashes.has(tokenHash)) {
      continue;
    }

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

  return matches;
}

export function scanText(text, filePath, rules = PUBLIC_HYGIENE_RULES) {
  const matches = [];

  for (const rule of rules) {
    if (rule.pattern) {
      matches.push(...buildPatternMatches(rule, text, filePath));
      continue;
    }

    if (rule.tokenHashes) {
      matches.push(...buildTokenHashMatches(rule, text, filePath));
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
