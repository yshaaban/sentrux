function escapeMarkdownCell(value) {
  if (value === null || value === undefined) {
    return '';
  }

  return String(value)
    .replace(/\|/g, '\\|')
    .replace(/\r?\n/g, '<br>');
}

function formatCloneEvidenceSummary(sample) {
  const cloneEvidence = sample.clone_evidence;
  if (!cloneEvidence) {
    return '';
  }

  const parts = [];
  if (Array.isArray(cloneEvidence.files) && cloneEvidence.files.length > 0) {
    parts.push(`files=${cloneEvidence.files.join(', ')}`);
  }
  if (Array.isArray(cloneEvidence.instances) && cloneEvidence.instances.length > 0) {
    parts.push(
      `instance lines=${cloneEvidence.instances
        .map((instance) => `${instance.file ?? 'unknown'}:${instance.lines ?? 'n/a'}`)
        .join(', ')}`,
    );
  }
  if (cloneEvidence.total_lines !== undefined) {
    parts.push(`total lines=${cloneEvidence.total_lines}`);
  }
  if (cloneEvidence.max_lines !== undefined) {
    parts.push(`max lines=${cloneEvidence.max_lines}`);
  }
  if (
    Array.isArray(cloneEvidence.recent_edit_reasons) &&
    cloneEvidence.recent_edit_reasons.length > 0
  ) {
    parts.push(`recent-edit reasons=${cloneEvidence.recent_edit_reasons.join(' | ')}`);
  }
  if (cloneEvidence.asymmetric_recent_change !== undefined) {
    parts.push(`asymmetric_recent_change=${cloneEvidence.asymmetric_recent_change}`);
  }

  return parts.join('; ');
}

function formatSampleEvidence(sample) {
  if (sample.clone_evidence) {
    return formatCloneEvidenceSummary(sample);
  }

  if (Array.isArray(sample.evidence) && sample.evidence.length > 0) {
    return sample.evidence.join(' · ');
  }

  return '';
}

function packetTitle(tool) {
  switch (tool) {
    case 'findings':
      return '# Findings Review Packet';
    case 'session_end':
      return '# Session End Review Packet';
    default:
      return '# Check Review Packet';
  }
}

function formatScanMetadataLines(packet) {
  const scanMetadata = packet.scan_metadata ?? null;
  const confidence = scanMetadata?.confidence ?? packet.confidence ?? null;
  const scanTrust = scanMetadata?.scan_trust ?? packet.scan_trust ?? null;

  if (!confidence && !scanTrust) {
    return [];
  }

  const lines = ['- scan trust / coverage:'];
  if (scanMetadata?.source_label || scanMetadata?.snapshot_label) {
    const sourceParts = [];
    if (scanMetadata.source_label) {
      sourceParts.push(scanMetadata.source_label);
    }
    if (scanMetadata.snapshot_label) {
      sourceParts.push(scanMetadata.snapshot_label);
    }
    lines.push(`  - source: \`${sourceParts.join(' / ')}\``);
  }
  if (scanTrust) {
    const keptFiles = scanTrust.kept_files ?? 'n/a';
    const candidateFiles = scanTrust.candidate_files ?? 'n/a';
    const trackedCandidates = scanTrust.tracked_candidates ?? 'n/a';
    const untrackedCandidates = scanTrust.untracked_candidates ?? 'n/a';
    lines.push(`  - kept files: \`${keptFiles} / ${candidateFiles}\``);
    lines.push(`  - tracked candidates: \`${trackedCandidates}\``);
    lines.push(`  - untracked candidates: \`${untrackedCandidates}\``);
    lines.push(`  - scan mode: \`${scanTrust.mode ?? 'n/a'}\``);
    lines.push(`  - scope coverage: \`${scanTrust.scope_coverage_0_10000 ?? 'n/a'} / 10000\``);
    lines.push(`  - overall confidence: \`${scanTrust.overall_confidence_0_10000 ?? 'n/a'} / 10000\``);
    lines.push(`  - partial: \`${scanTrust.partial ?? 'n/a'}\``);
    lines.push(`  - truncated: \`${scanTrust.truncated ?? 'n/a'}\``);
    lines.push(`  - fallback reason: \`${scanTrust.fallback_reason ?? 'n/a'}\``);
    if (scanTrust.exclusions) {
      const exclusions = scanTrust.exclusions;
      const exclusionParts = [];
      if (exclusions.total !== undefined) {
        exclusionParts.push(`${exclusions.total} total`);
      }
      if (exclusions.bucketed?.vendor !== undefined) {
        exclusionParts.push(`${exclusions.bucketed.vendor} vendor`);
      }
      if (exclusions.bucketed?.generated !== undefined) {
        exclusionParts.push(`${exclusions.bucketed.generated} generated`);
      }
      if (exclusions.bucketed?.build !== undefined) {
        exclusionParts.push(`${exclusions.bucketed.build} build`);
      }
      if (exclusions.bucketed?.fixture !== undefined) {
        exclusionParts.push(`${exclusions.bucketed.fixture} fixture`);
      }
      if (exclusions.bucketed?.cache !== undefined) {
        exclusionParts.push(`${exclusions.bucketed.cache} cache`);
      }
      if (exclusions.ignored_extension !== undefined) {
        exclusionParts.push(`${exclusions.ignored_extension} ignored_extension`);
      }
      if (exclusions.too_large !== undefined) {
        exclusionParts.push(`${exclusions.too_large} too_large`);
      }
      if (exclusions.metadata_error !== undefined) {
        exclusionParts.push(`${exclusions.metadata_error} metadata_error`);
      }
      if (exclusionParts.length > 0) {
        lines.push(`  - exclusions: \`${exclusionParts.join(', ')}\``);
      }
    }
    if (scanTrust.resolution) {
      lines.push(
        `  - resolution: \`${scanTrust.resolution.resolved ?? 'n/a'} resolved, ${scanTrust.resolution.unresolved_internal ?? 'n/a'} internal unresolved, ${scanTrust.resolution.unresolved_external ?? 'n/a'} external unresolved, ${scanTrust.resolution.unresolved_unknown ?? 'n/a'} unknown unresolved\``,
      );
      lines.push(
        `  - internal resolution confidence: \`${scanTrust.resolution.internal_confidence_0_10000 ?? 'n/a'} / 10000\``,
      );
    }
  }
  if (confidence) {
    lines.push(`  - scan confidence: \`${confidence.scan_confidence_0_10000 ?? 'n/a'} / 10000\``);
    lines.push(`  - rule coverage: \`${confidence.rule_coverage_0_10000 ?? 'n/a'} / 10000\``);
    lines.push(`  - semantic rules loaded: \`${confidence.semantic_rules_loaded ?? 'n/a'}\``);
  }

  return lines;
}

export function formatPacketMarkdown(packet) {
  const lines = [];
  lines.push(packetTitle(packet.tool));
  lines.push('');
  lines.push(`- repo root: \`${packet.repo_root}\``);
  lines.push(`- tool: \`${packet.tool}\``);
  lines.push(`- source mode: \`${packet.source_mode ?? 'repo-head'}\``);
  if (Array.isArray(packet.filters?.kinds) && packet.filters.kinds.length > 0) {
    lines.push(`- filtered kinds: \`${packet.filters.kinds.join('`, `')}\``);
  }
  if (Array.isArray(packet.source_paths) && packet.source_paths.length > 0) {
    lines.push(`- source path(s):`);
    for (const sourcePath of packet.source_paths) {
      lines.push(`  - \`${sourcePath}\``);
    }
  }
  lines.push(...formatScanMetadataLines(packet));
  lines.push(`- generated at: \`${packet.generated_at}\``);
  lines.push(`- sample count: ${packet.samples.length}`);
  if (packet.summary?.repair_packet_complete_rate !== undefined) {
    lines.push(
      `- repair-packet completeness: \`${packet.summary.repair_packet_complete_count ?? 0}/${packet.summary.sample_count ?? packet.samples.length}\` (${packet.summary.repair_packet_complete_rate ?? 'n/a'})`,
    );
    lines.push(
      `- top-3 repair-packet completeness: \`${packet.summary.top_3_repair_packet_complete_rate ?? 'n/a'}\``,
    );
    lines.push(
      `- top-10 repair-packet completeness: \`${packet.summary.top_10_repair_packet_complete_rate ?? 'n/a'}\``,
    );
  }
  if (Array.isArray(packet.summary?.kind_counts) && packet.summary.kind_counts.length > 0) {
    lines.push(
      `- kind counts: ${packet.summary.kind_counts.map((entry) => `${entry.kind}=${entry.count}`).join(', ')}`,
    );
  }
  lines.push('');
  lines.push('| Review ID | Kind | Source | Snapshot | Rank | Scope | Severity | Summary | Evidence | Classification | Action |');
  lines.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |');
  for (const sample of packet.samples) {
    lines.push(
      `| \`${escapeMarkdownCell(sample.review_id)}\` | \`${escapeMarkdownCell(sample.kind ?? 'unknown')}\` | \`${escapeMarkdownCell(sample.source_label ?? sample.source_kind ?? 'unknown')}\` | \`${escapeMarkdownCell(sample.snapshot_label ?? 'n/a')}\` | ${escapeMarkdownCell(sample.rank ?? 'n/a')} | \`${escapeMarkdownCell(sample.scope ?? 'unknown')}\` | \`${escapeMarkdownCell(sample.severity ?? 'unknown')}\` | ${escapeMarkdownCell(sample.summary ?? '')} | ${escapeMarkdownCell(formatSampleEvidence(sample))} |  |  |`,
    );
  }
  lines.push('');
  return `${lines.join('\n')}\n`;
}
