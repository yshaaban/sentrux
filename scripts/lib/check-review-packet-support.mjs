import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { REVIEW_PACKET_COMPLETENESS_POLICY } from './signal-calibration-policy.mjs';

function loadJson(targetPath) {
  return readFile(targetPath, 'utf8').then((source) => JSON.parse(source));
}

function sourceLabelFromPath(targetPath) {
  const baseName = path.basename(targetPath);
  return baseName.endsWith('.json') ? baseName.slice(0, -5) : baseName;
}

function isPlainObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function hasText(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function uniqueStrings(values) {
  return [...new Set(values.filter(hasText))];
}

function selectRawSamples(tool, payload) {
  if (tool === 'findings') {
    return payload.findings ?? [];
  }
  if (tool === 'session_end') {
    return payload.introduced_findings ?? [];
  }
  return payload.actions ?? payload.issues ?? [];
}

function normalizeCloneInstance(instance) {
  if (!isPlainObject(instance)) {
    return null;
  }

  return {
    file: instance.file ?? null,
    func: instance.func ?? null,
    lines: instance.lines ?? null,
    commit_count: instance.commit_count ?? null,
  };
}

function buildCloneEvidence(sample) {
  if (!sample || typeof sample !== 'object') {
    return null;
  }
  if (typeof sample.kind !== 'string' || !sample.kind.includes('clone')) {
    return null;
  }

  const cloneFiles = Array.isArray(sample.files)
    ? sample.files.filter((file) => typeof file === 'string')
    : [];
  const cloneInstances = Array.isArray(sample.instances)
    ? sample.instances.map(normalizeCloneInstance).filter(Boolean)
    : [];
  const cloneReasons = Array.isArray(sample.reasons)
    ? sample.reasons.filter((reason) => typeof reason === 'string' && reason.length > 0)
    : [];
  const cloneEvidence = {};

  if (cloneFiles.length > 0) {
    cloneEvidence.files = cloneFiles;
  }
  if (cloneInstances.length > 0) {
    cloneEvidence.instances = cloneInstances;
  }
  if (sample.total_lines !== undefined) {
    cloneEvidence.total_lines = sample.total_lines;
  }
  if (sample.max_lines !== undefined) {
    cloneEvidence.max_lines = sample.max_lines;
  }
  if (sample.recently_touched_file_count !== undefined) {
    cloneEvidence.recently_touched_file_count = sample.recently_touched_file_count;
  }
  if (sample.production_instance_count !== undefined) {
    cloneEvidence.production_instance_count = sample.production_instance_count;
  }
  if (sample.asymmetric_recent_change !== undefined) {
    cloneEvidence.asymmetric_recent_change = sample.asymmetric_recent_change;
  }
  if (cloneReasons.length > 0) {
    cloneEvidence.recent_edit_reasons = cloneReasons;
  }

  return Object.keys(cloneEvidence).length > 0 ? cloneEvidence : null;
}

function buildLikelyFixSites(sample, scope) {
  const likelyFixSites = [];

  for (const candidate of sample.likely_fix_sites ?? []) {
    if (hasText(candidate)) {
      likelyFixSites.push(candidate);
    }
  }
  if (hasText(sample.file)) {
    likelyFixSites.push(sample.file);
  }
  if (likelyFixSites.length === 0 && hasText(scope) && !scope.startsWith('cycle:')) {
    likelyFixSites.push(scope);
  }

  return uniqueStrings(likelyFixSites);
}

function buildRepairPacket(sample, scope, summary, evidence, expectedFixSurface) {
  const likelyFixSites = buildLikelyFixSites(sample, scope);
  const fixHint = hasText(sample.fix_hint) ? sample.fix_hint : null;
  const inspectionFocus = uniqueStrings(sample.inspection_focus ?? []);
  const repairSurface = hasText(expectedFixSurface)
    ? expectedFixSurface
    : likelyFixSites.length > 0
      ? 'concrete_fix_site'
      : null;
  const requiredFieldState = {
    scope: hasText(scope),
    summary: hasText(summary),
    evidence: evidence.length > 0,
    repair_surface: hasText(repairSurface) || hasText(fixHint),
  };
  const preferredFieldState = {
    fix_hint: hasText(fixHint),
    likely_fix_sites: likelyFixSites.length > 0,
  };
  const complete = REVIEW_PACKET_COMPLETENESS_POLICY.requiredFields.every(
    (field) => requiredFieldState[field],
  );
  const satisfiedFieldCount = [
    ...REVIEW_PACKET_COMPLETENESS_POLICY.requiredFields.map(
      (field) => requiredFieldState[field],
    ),
    ...REVIEW_PACKET_COMPLETENESS_POLICY.preferredFields.map(
      (field) => preferredFieldState[field],
    ),
  ].filter(Boolean).length;
  const totalTrackedFieldCount =
    REVIEW_PACKET_COMPLETENESS_POLICY.requiredFields.length +
    REVIEW_PACKET_COMPLETENESS_POLICY.preferredFields.length;
  const missingFields = [
    ...REVIEW_PACKET_COMPLETENESS_POLICY.requiredFields.filter(
      (field) => !requiredFieldState[field],
    ),
    ...REVIEW_PACKET_COMPLETENESS_POLICY.preferredFields.filter(
      (field) => !preferredFieldState[field],
    ),
  ];

  return {
    complete,
    completeness_0_10000: Math.round((satisfiedFieldCount / totalTrackedFieldCount) * 10000),
    missing_fields: missingFields,
    required_fields: requiredFieldState,
    preferred_fields: preferredFieldState,
    fix_hint: fixHint,
    likely_fix_sites: likelyFixSites,
    inspection_focus: inspectionFocus,
    expected_fix_surface: repairSurface,
  };
}

function buildScanMetadata(payload, sourceKind, sourceLabel, snapshotLabel, scanPayload = null) {
  const confidence = isPlainObject(payload?.confidence)
    ? payload.confidence
    : isPlainObject(scanPayload?.confidence)
      ? scanPayload.confidence
      : null;
  const scanTrust = isPlainObject(payload?.scan_trust)
    ? payload.scan_trust
    : isPlainObject(scanPayload?.scan_trust)
      ? scanPayload.scan_trust
      : null;

  if (!confidence && !scanTrust) {
    return null;
  }

  return {
    source_kind: sourceKind,
    source_label: sourceLabel,
    snapshot_label: snapshotLabel ?? null,
    confidence: confidence ?? null,
    scan_trust: scanTrust ?? null,
  };
}

function collectArtifactMetadata(tool, entries) {
  for (const entry of entries) {
    const bundleMetadata = buildScanMetadata(entry.bundle, entry.source_kind, entry.source_label, 'bundle');
    if (bundleMetadata) {
      return bundleMetadata;
    }

    const payloadEntry = selectArtifactPayload(tool, entry.bundle);
    if (!payloadEntry) {
      continue;
    }

    const metadata = buildScanMetadata(
      payloadEntry.payload,
      entry.source_kind,
      entry.source_label,
      payloadEntry.snapshot_label,
    );
    if (metadata) {
      return metadata;
    }
  }

  return null;
}

function collectSelectedArtifactMetadata(selection) {
  if (!selection) {
    return null;
  }

  const payloadMetadata = buildScanMetadata(
    selection.payloadEntry.payload,
    selection.entry.source_kind,
    selection.entry.source_label,
    selection.payloadEntry.snapshot_label,
  );
  if (payloadMetadata) {
    return payloadMetadata;
  }

  return buildScanMetadata(
    selection.entry.bundle,
    selection.entry.source_kind,
    selection.entry.source_label,
    'bundle',
  );
}

function selectCheckPayloads(bundle) {
  const payloads = [];
  if (bundle.initial_check) {
    payloads.push({ snapshot_label: 'initial_check', payload: bundle.initial_check });
  }
  for (const snapshot of bundle.snapshots ?? []) {
    if (snapshot.check) {
      payloads.push({
        snapshot_label: snapshot.label ?? 'snapshot',
        payload: snapshot.check,
      });
    }
  }
  if (bundle.final_check) {
    payloads.push({ snapshot_label: 'final_check', payload: bundle.final_check });
  }

  return payloads;
}

function selectArtifactPayload(tool, bundle) {
  if (tool === 'check') {
    const payloads = selectCheckPayloads(bundle);
    for (const payloadEntry of payloads) {
      if (selectRawSamples(tool, payloadEntry.payload).length > 0) {
        return payloadEntry;
      }
    }

    return payloads.at(-1) ?? null;
  }
  if (tool === 'findings') {
    return bundle.findings ? { snapshot_label: 'findings', payload: bundle.findings } : null;
  }
  if (tool === 'session_end') {
    return bundle.session_end
      ? { snapshot_label: 'session_end', payload: bundle.session_end }
      : null;
  }

  return null;
}

function extractSamples(tool, payloadEntry, sourceEntry) {
  const rawSamples = selectRawSamples(tool, payloadEntry.payload);

  return rawSamples.map(function toSample(sample, index) {
    const cloneEvidence = buildCloneEvidence(sample);
    const summary = sample.summary ?? sample.message ?? null;
    const evidence = Array.isArray(sample.evidence) ? sample.evidence : [];
    const scope =
      sample.scope ??
      sample.file ??
      cloneEvidence?.files?.slice(0, 2).join(' | ') ??
      cloneEvidence?.instances?.[0]?.file ??
      null;
    const repairPacket = buildRepairPacket(
      sample,
      scope,
      summary,
      evidence,
      sourceEntry.expected_fix_surface,
    );

    return {
      review_id: `${tool}-${index + 1}`,
      rank: index + 1,
      kind: sample.kind ?? null,
      report_bucket: tool === 'check' ? 'actions' : tool,
      scope,
      severity: sample.severity ?? null,
      summary,
      evidence,
      fix_hint: repairPacket.fix_hint,
      likely_fix_sites: repairPacket.likely_fix_sites,
      inspection_focus: repairPacket.inspection_focus,
      repair_packet: repairPacket,
      source_kind: sourceEntry.source_kind,
      source_label: sourceEntry.source_label,
      snapshot_label: payloadEntry.snapshot_label,
      task_id: sourceEntry.task_id,
      task_label: sourceEntry.task_label,
      replay_id: sourceEntry.replay_id,
      commit: sourceEntry.commit,
      output_dir: sourceEntry.output_dir,
      expected_signal_kinds: sourceEntry.expected_signal_kinds,
      expected_fix_surface: sourceEntry.expected_fix_surface,
      clone_evidence: cloneEvidence,
      classification: null,
      notes: '',
      action: '',
    };
  });
}

function filterSamplesByKinds(samples, kinds) {
  const normalizedKinds = new Set(kinds ?? []);
  if (normalizedKinds.size === 0) {
    return samples;
  }

  return samples.filter((sample) => normalizedKinds.has(sample.kind));
}

function limitSamples(samples, limit) {
  return samples.slice(0, Math.max(limit, 1));
}

function buildPacketSamplesFromEntries(tool, entries, limit, kinds) {
  const samples = [];
  let metadataSelection = null;

  for (const entry of entries) {
    const payloadEntry = selectArtifactPayload(tool, entry.bundle);
    if (!payloadEntry) {
      continue;
    }

    const entrySamples = filterSamplesByKinds(extractSamples(tool, payloadEntry, entry), kinds);
    if (entrySamples.length > 0 && !metadataSelection) {
      metadataSelection = {
        entry,
        payloadEntry,
      };
    }
    for (const sample of entrySamples) {
      samples.push(sample);
      if (samples.length >= Math.max(limit, 1)) {
        return {
          samples,
          metadataSelection,
        };
      }
    }
  }

  return {
    samples,
    metadataSelection,
  };
}

function renumberSamples(tool, samples) {
  return samples.map((sample, index) => ({
    ...sample,
    rank: index + 1,
    review_id: `${tool}-${index + 1}`,
  }));
}

function buildPacketSummary(samples) {
  const kindCounts = new Map();
  const top3Samples = samples.slice(0, 3);
  const top10Samples = samples.slice(0, 10);

  for (const sample of samples) {
    const key = sample.kind ?? 'unknown';
    kindCounts.set(key, (kindCounts.get(key) ?? 0) + 1);
  }

  return {
    sample_count: samples.length,
    repair_packet_complete_count: samples.filter((sample) => sample.repair_packet?.complete).length,
    repair_packet_complete_rate: samples.length
      ? samples.filter((sample) => sample.repair_packet?.complete).length / samples.length
      : null,
    top_3_repair_packet_complete_rate: top3Samples.length
      ? top3Samples.filter((sample) => sample.repair_packet?.complete).length / top3Samples.length
      : null,
    top_10_repair_packet_complete_rate: top10Samples.length
      ? top10Samples.filter((sample) => sample.repair_packet?.complete).length / top10Samples.length
      : null,
    kind_counts: [...kindCounts.entries()]
      .map(([kind, count]) => ({ kind, count }))
      .sort((left, right) => right.count - left.count || left.kind.localeCompare(right.kind)),
  };
}

function buildPacket(args, repoRootValue, sourceMode, sourcePaths, samples, scanMetadata = null) {
  const packet = {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    repo_root: repoRootValue,
    tool: args.tool,
    source_mode: sourceMode,
    source_paths: sourcePaths,
    filters: {
      kinds: args.kinds,
    },
    summary: buildPacketSummary(samples),
    samples,
  };

  if (scanMetadata) {
    packet.scan_metadata = scanMetadata;
  }

  return packet;
}

function createRepoHeadEntry() {
  return {
    source_kind: 'repo-head',
    source_label: 'repo-head',
    task_id: null,
    task_label: null,
    replay_id: null,
    commit: null,
    output_dir: null,
    expected_signal_kinds: [],
    expected_fix_surface: null,
  };
}

function buildVerdictTemplateData(packet, sourceReport) {
  return {
    repo: packet.repo_root ? path.basename(packet.repo_root) : 'unknown',
    captured_at: packet.generated_at,
    source_report: sourceReport,
    source_feedback:
      'Replace the placeholder verdict values below after reviewing the packet. Keep verdict order rank-preserving because top-1/top-3/top-10 actionable precision is computed from this order. Do not use this template as scored evidence until it has been curated by a reviewer.',
    verdicts: packet.samples.map((sample) => ({
      scope: sample.scope ?? sample.source_label ?? 'unknown-scope',
      kind: sample.kind ?? 'unknown-kind',
      report_bucket: sample.report_bucket,
      category: 'useful',
      expected_trust_tier: sample.severity === 'high' ? 'trusted' : 'watchpoint',
      expected_presentation_class: 'review_required',
      expected_leverage_class: sample.expected_fix_surface ?? 'local_refactor_target',
      expected_summary_presence: sample.rank <= 3 ? 'headline' : 'section_present',
      preferred_over: [],
      engineer_note:
        sample.repair_packet?.complete === false
          ? `${sample.summary ?? 'Replace with reviewer rationale.'} Confirm usefulness, rank, and whether missing repair guidance (${sample.repair_packet.missing_fields.join(', ')}) keeps this out of the primary surface.`
          : sample.summary ?? 'Replace with reviewer rationale.',
      expected_v2_behavior:
        sample.repair_packet?.complete === false
          ? `Confirm the ranking and presentation for ${sample.kind ?? 'this finding'}, and add explicit repair guidance before treating it as promotion-grade lead evidence.`
          : `Confirm the ranking and presentation for ${sample.kind ?? 'this finding'}.`,
    })),
  };
}

function escapeMarkdownCell(value) {
  if (value === null || value === undefined) {
    return '';
  }

  return String(value)
    .replace(/\|/g, '\\|')
    .replace(/\r?\n/g, '<br>');
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
  if (Array.isArray(cloneEvidence.recent_edit_reasons) && cloneEvidence.recent_edit_reasons.length > 0) {
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

export async function loadArtifactInput(args) {
  const sources = [];
  if (args.bundlePath) {
    const bundle = await loadJson(args.bundlePath);
    sources.push({
      source_mode: 'bundle',
      source_paths: [path.resolve(args.bundlePath)],
      repo_root: bundle.repo_root ?? bundle.source_root ?? null,
      label: sourceLabelFromPath(args.bundlePath),
      entries: [
        {
          bundle,
          bundle_path: path.resolve(args.bundlePath),
          output_dir: path.dirname(path.resolve(args.bundlePath)),
          source_kind: 'bundle',
          source_label:
            bundle.task_label ??
            bundle.task_id ??
            bundle.replay_id ??
            bundle.replay?.commit ??
            sourceLabelFromPath(args.bundlePath),
          task_id: bundle.task_id ?? null,
          task_label: bundle.task_label ?? null,
          replay_id: bundle.replay_id ?? null,
          commit: bundle.replay?.commit ?? null,
          expected_signal_kinds: bundle.expected_signal_kinds ?? [],
          expected_fix_surface: bundle.expected_fix_surface ?? null,
        },
      ],
    });
  }
  if (args.codexBatchPath) {
    sources.push(await loadBatchArtifact(args.codexBatchPath, 'codex-batch'));
  }
  if (args.replayBatchPath) {
    sources.push(await loadBatchArtifact(args.replayBatchPath, 'replay-batch'));
  }

  if (sources.length === 0) {
    return null;
  }
  if (sources.length === 1) {
    return sources[0];
  }

  return {
    source_mode: 'combined',
    source_paths: [...new Set(sources.flatMap((source) => source.source_paths))],
    repo_root: sources[0]?.repo_root ?? null,
    label: sources.map((source) => source.label).join('-'),
    entries: sources.flatMap((source) => source.entries),
  };
}

async function loadBatchArtifact(batchPath, kind) {
  const batch = await loadJson(batchPath);
  const batchDir = path.dirname(path.resolve(batchPath));
  const bundleFileName = kind === 'codex-batch' ? 'codex-session.json' : 'diff-replay.json';
  const bundlePaths = [];
  const entries = [];

  for (const result of batch.results ?? []) {
    const outputDir = result.output_dir ? path.resolve(batchDir, result.output_dir) : null;
    if (!outputDir) {
      throw new Error(`Missing output_dir for batch result in ${batchPath}`);
    }

    const bundlePath = path.join(outputDir, bundleFileName);
    if (!existsSync(bundlePath)) {
      throw new Error(`Missing bundle artifact: ${bundlePath}`);
    }

    bundlePaths.push(bundlePath);
    const bundle = await loadJson(bundlePath);
    entries.push({
      bundle,
      bundle_path: bundlePath,
      output_dir: outputDir,
      source_kind: kind,
      source_label:
        result.task_label ??
        result.task_id ??
        result.replay_id ??
        result.commit ??
        bundle.task_label ??
        bundle.task_id ??
        bundle.replay_id ??
        bundle.replay?.commit ??
        sourceLabelFromPath(bundlePath),
      task_id: result.task_id ?? bundle.task_id ?? null,
      task_label: result.task_label ?? bundle.task_label ?? null,
      replay_id: result.replay_id ?? bundle.replay_id ?? null,
      commit: result.commit ?? bundle.replay?.commit ?? null,
      expected_signal_kinds: result.expected_signal_kinds ?? bundle.expected_signal_kinds ?? [],
      expected_fix_surface: result.expected_fix_surface ?? bundle.expected_fix_surface ?? null,
    });
  }

  return {
    source_mode: kind,
    source_paths: [path.resolve(batchPath), ...bundlePaths],
    repo_root: batch.repo_root ?? entries[0]?.bundle?.repo_root ?? null,
    label: sourceLabelFromPath(batchPath),
    entries,
  };
}

export function buildPacketFromArtifactInput(args, source) {
  const repoRootValue = source.repo_root ?? args.repoRoot;
  const sampleSelection = buildPacketSamplesFromEntries(
    args.tool,
    source.entries,
    args.limit,
    args.kinds,
  );
  const scanMetadata =
    collectSelectedArtifactMetadata(sampleSelection.metadataSelection) ??
    collectArtifactMetadata(args.tool, source.entries);
  const samples = renumberSamples(args.tool, sampleSelection.samples);

  return buildPacket(
    args,
    repoRootValue,
    source.source_mode,
    source.source_paths,
    samples,
    scanMetadata,
  );
}

export function buildPacketFromRepoHeadPayload(args, payload, scanPayload = null) {
  const repoHeadEntry = createRepoHeadEntry();
  const scanMetadata = buildScanMetadata(
    payload,
    'repo-head',
    'repo-head',
    'repo_head',
    scanPayload,
  );
  const samples = renumberSamples(
    args.tool,
    limitSamples(
      filterSamplesByKinds(
        extractSamples(
          args.tool,
          { snapshot_label: 'repo_head', payload },
          repoHeadEntry,
        ),
        args.kinds,
      ),
      args.limit,
    ),
  );

  return buildPacket(args, args.repoRoot, 'repo-head', [], samples, scanMetadata);
}

export function buildVerdictTemplate(packet, sourceReport) {
  return buildVerdictTemplateData(packet, sourceReport);
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
    lines.push(`- kind counts: ${packet.summary.kind_counts.map((entry) => `${entry.kind}=${entry.count}`).join(', ')}`);
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
