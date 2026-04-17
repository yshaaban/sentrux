import path from 'node:path';

export function formatUtcDate(timestamp) {
  const date = new Date(timestamp);
  return new Intl.DateTimeFormat('en-US', {
    month: 'long',
    day: 'numeric',
    year: 'numeric',
    timeZone: 'UTC',
  }).format(date);
}

export function formatIdentity(metadata) {
  const identity = metadata?.source_tree_identity ?? {};
  return {
    analysis_mode: metadata?.analysis_mode ?? identity.analysis_mode ?? 'unknown',
    commit: identity.commit ?? 'unknown',
    dirty_paths_count: identity.dirty_paths_count ?? 'unknown',
    dirty_paths: identity.dirty_paths ?? [],
    dirty_paths_fingerprint: identity.dirty_paths_fingerprint ?? 'unknown',
    tree_fingerprint: identity.tree_fingerprint ?? 'unknown',
  };
}

export function appendCodeBullet(lines, label, value) {
  lines.push(`- ${label}: \`${value}\``);
}

export function appendCodeList(lines, title, values) {
  if ((values ?? []).length === 0) {
    return;
  }

  lines.push(`- ${title}:`);
  for (const value of values) {
    lines.push(`  - \`${value}\``);
  }
}

export function formatRepoPathMarkdown(repoPath, targetPath) {
  return `[${path.basename(targetPath)}](${path.join(repoPath, targetPath)})`;
}

function looksLikeRepoPath(value) {
  if (typeof value !== 'string') {
    return false;
  }

  const repoPrefixes = ['src/', 'server/', 'electron/', 'scripts/', 'docs/'];
  return repoPrefixes.some((prefix) => value.startsWith(prefix));
}

function isSingleRepoPath(value) {
  return looksLikeRepoPath(value) && !value.includes('|');
}

export function formatScopeHeading(repoPath, scope) {
  return isSingleRepoPath(scope) ? formatRepoPathMarkdown(repoPath, scope) : scope;
}

export function formatScopeBullet(repoPath, scope) {
  return isSingleRepoPath(scope) ? formatRepoPathMarkdown(repoPath, scope) : `\`${scope}\``;
}

export function appendRepoLinkList(lines, title, repoPath, surfaces, limit = 5) {
  if ((surfaces ?? []).length === 0) {
    return;
  }

  lines.push(`- ${title}:`);
  for (const surface of surfaces.slice(0, limit)) {
    lines.push(`  - ${formatRepoPathMarkdown(repoPath, surface)}`);
  }
}

export function appendScanCoverage(
  lines,
  scan,
  { includeBuckets = false, includeSessionBaseline = false } = {},
) {
  const exclusionBuckets =
    scan.scan_trust?.exclusions?.by_category ?? scan.scan_trust?.exclusions?.bucketed;

  appendCodeBullet(lines, 'scanned files', scan.files ?? 'n/a');
  appendCodeBullet(lines, 'scanned lines', scan.lines ?? 'n/a');
  appendCodeBullet(
    lines,
    'kept files from git candidate set',
    `${scan.scan_trust?.kept_files ?? 'n/a'} / ${scan.scan_trust?.candidate_files ?? 'n/a'}`,
  );
  appendCodeBullet(lines, 'excluded files', scan.scan_trust?.exclusions?.total ?? 'n/a');
  if (includeBuckets && exclusionBuckets) {
    lines.push('- excluded buckets:');
    for (const [bucket, count] of Object.entries(exclusionBuckets)) {
      lines.push(`  - ${bucket}: \`${count}\``);
    }
  }
  appendCodeBullet(lines, 'resolved imports', scan.scan_trust?.resolution?.resolved ?? 'n/a');
  appendCodeBullet(
    lines,
    'unresolved internal imports',
    scan.scan_trust?.resolution?.unresolved_internal ?? 'n/a',
  );
  appendCodeBullet(
    lines,
    'unresolved external imports',
    scan.scan_trust?.resolution?.unresolved_external ?? 'n/a',
  );
  appendCodeBullet(
    lines,
    'unresolved unknown imports',
    scan.scan_trust?.resolution?.unresolved_unknown ?? 'n/a',
  );
  appendCodeBullet(
    lines,
    'scan confidence',
    `${scan.confidence?.scan_confidence_0_10000 ?? 'n/a'} / 10000`,
  );
  appendCodeBullet(
    lines,
    'rule coverage',
    `${scan.confidence?.rule_coverage_0_10000 ?? 'n/a'} / 10000`,
  );
  appendCodeBullet(
    lines,
    'semantic rules loaded',
    scan.confidence?.semantic_rules_loaded ? 'true' : 'false',
  );
  if (includeSessionBaseline) {
    appendCodeBullet(
      lines,
      'session baseline loaded in `findings`',
      scan.session_baseline_loaded ? 'true' : 'false',
    );
  }
}

export function finalizeMarkdown(lines) {
  const output = [...lines];
  while (output.length > 0 && output.at(-1) === '') {
    output.pop();
  }

  return output.join('\n');
}

export function buildReportHeading(headCloneAnalysis) {
  if (headCloneAnalysis) {
    return '# Parallel Code: Committed HEAD Analysis Report For Engineers';
  }

  return '# Parallel Code: Live Analysis Report For Engineers';
}

export function buildAppendixHeading(headCloneAnalysis) {
  if (headCloneAnalysis) {
    return '# Parallel Code: Committed HEAD Analysis Report Appendix';
  }

  return '# Parallel Code: Live Analysis Report Appendix';
}

export function buildAnalysisGeneratedLine(snapshot, metadata, headCloneAnalysis) {
  if (headCloneAnalysis) {
    return `Generated on ${formatUtcDate(snapshot.generated_at)} from a committed HEAD clone of \`${metadata.parallel_code_root}\`.`;
  }

  return `Generated on ${formatUtcDate(snapshot.generated_at)} from the live checkout at \`${metadata.parallel_code_root}\`.`;
}

export function appendAnalysisHeader(lines, heading, generatedLine, audienceLines) {
  lines.push(heading);
  lines.push('');
  lines.push(generatedLine);
  lines.push('');
  for (const audienceLine of audienceLines) {
    lines.push(audienceLine);
  }
  lines.push('');
}

export function appendFreshnessGateSection(lines, freshness, allowStale) {
  lines.push('## Freshness Gate');
  lines.push('');
  lines.push(`- analysis mode: \`${freshness.analysis_mode}\``);
  lines.push(`- commit: \`${freshness.commit}\``);
  lines.push(`- dirty paths: \`${freshness.dirty_paths_count}\``);
  lines.push(`- dirty-path fingerprint: \`${freshness.dirty_paths_fingerprint}\``);
  lines.push(`- tree fingerprint: \`${freshness.tree_fingerprint}\``);
  lines.push(
    `- stale goldens: ${allowStale ? 'accepted via override' : 'refused by default unless the goldens are fresh'}`,
  );
  lines.push('');
}

export function appendAnalyzedScopeSection(
  lines,
  { metadata, snapshotMarkdownPath, benchmarkPath, headCloneAnalysis, liveIdentity },
) {
  lines.push('## What Was Analyzed');
  lines.push('');
  lines.push(`- live source checkout: \`${metadata.parallel_code_root}\``);
  if (headCloneAnalysis) {
    lines.push('- report scope: committed `HEAD` only');
    lines.push(
      `- ignored working-tree changes outside HEAD: \`${liveIdentity?.dirty_paths?.length ?? 0}\``,
    );
  }
  lines.push(`- rules file used for the run: \`${metadata.rules_source}\``);
  lines.push(`- comparison snapshot: \`${snapshotMarkdownPath}\``);
  lines.push(`- benchmark artifact: \`${benchmarkPath}\``);
  lines.push('');
}

export function appendSourceDocumentsSection(lines, snapshotMarkdownPath, goldenDir) {
  lines.push('## Source Documents');
  lines.push('');
  lines.push(`- proof snapshot: \`${snapshotMarkdownPath}\``);
  lines.push(`- golden metadata: \`${path.join(goldenDir, 'metadata.json')}\``);
}

export function appendAppendixMethodSection(lines, metadata, repoRoot, headCloneAnalysis) {
  lines.push('## Method');
  lines.push('');
  lines.push('The analysis used:');
  lines.push('');
  lines.push(`- live source repo: [${metadata.parallel_code_root}](${metadata.parallel_code_root})`);
  lines.push(`- bundled rules file: [parallel-code.rules.toml](${metadata.rules_source})`);
  lines.push(
    `- goldens refresh path: [refresh_parallel_code_goldens.sh](${path.join(repoRoot, 'scripts/refresh_parallel_code_goldens.sh')})`,
  );
  lines.push(
    `- current binary used for the run: [${metadata.sentrux_binary}](${metadata.sentrux_binary})`,
  );
  lines.push('');
  lines.push('Scope caveat:');
  lines.push('');
  lines.push('- the live repo has `.sentrux/baseline.json`');
  lines.push('- it does **not** currently have its own `.sentrux/rules.toml`');
  lines.push('- this run therefore still uses the bundled example rules');
  if (headCloneAnalysis) {
    lines.push('- this report intentionally ignores uncommitted working-tree changes');
  }
  lines.push('');
}

export function appendAppendixScanCoverageSection(lines, scan) {
  lines.push('## Scan Scope And Confidence');
  lines.push('');
  lines.push('Current scan:');
  lines.push('');
  appendScanCoverage(lines, scan, { includeBuckets: true, includeSessionBaseline: true });
  lines.push('');
}
