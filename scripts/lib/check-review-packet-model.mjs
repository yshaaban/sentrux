import path from 'node:path';

import { REVIEW_PACKET_COMPLETENESS_POLICY } from './signal-calibration-policy.mjs';
import {
  actionKindWeight,
  actionLeverageWeight,
  actionPresentationWeight,
} from './signal-policy.mjs';
import { buildStructuredReviewVerdictFieldsFromPacketSample } from './review-verdict-enrichment.mjs';

function isPlainObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function hasText(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function uniqueStrings(values) {
  return [...new Set(values.filter(hasText))];
}

function severityWeight(severity) {
  switch (severity) {
    case 'high':
      return 2;
    case 'medium':
    case 'watchpoint':
      return 1;
    case 'low':
      return 0;
    default:
      return 0;
  }
}

function sourceWeight(sample) {
  if (matchesSessionCloneKind(sample.kind)) {
    return 2;
  }
  if (sample.kind === 'touched_clone_family') {
    return 1;
  }

  if (sample.source === 'obligation') {
    return 5;
  }
  if (sample.source === 'rules' && sample.origin === 'explicit') {
    return 4;
  }
  if (sample.source === 'rules' && sample.origin === 'zero_config') {
    return 2;
  }
  if (sample.source === 'structural') {
    return 1;
  }

  return 0;
}

function matchesSessionCloneKind(kind) {
  return kind === 'session_introduced_clone' || kind === 'clone_propagation_drift';
}

function summarySurfaceLabel(summaryPresence) {
  switch (summaryPresence) {
    case 'headline':
      return 'lead surface';
    case 'side_channel':
      return 'side channel';
    default:
      return 'supporting surface';
  }
}

function trustTierWeight(trustTier) {
  switch (trustTier) {
    case 'trusted':
      return 3;
    case 'watchpoint':
      return 2;
    case 'experimental':
      return 1;
    default:
      return 0;
  }
}

function confidenceWeight(confidence) {
  switch (confidence) {
    case 'high':
      return 2;
    case 'medium':
      return 1;
    case 'experimental':
      return 0;
    default:
      return 0;
  }
}

function repairabilityWeight(sample) {
  return Math.min(Math.floor((sample.repair_packet?.completeness_0_10000 ?? 0) / 2000), 5);
}

function numericScore(value) {
  return typeof value === 'number' ? value : 0;
}

function comparePacketSamples(left, right) {
  return (
    sourceWeight(right) - sourceWeight(left) ||
    actionKindWeight(right.kind ?? '') - actionKindWeight(left.kind ?? '') ||
    severityWeight(right.severity) - severityWeight(left.severity) ||
    actionLeverageWeight(right.leverage_class ?? '') -
      actionLeverageWeight(left.leverage_class ?? '') ||
    actionPresentationWeight(right.presentation_class ?? '') -
      actionPresentationWeight(left.presentation_class ?? '') ||
    trustTierWeight(right.trust_tier) - trustTierWeight(left.trust_tier) ||
    confidenceWeight(right.confidence) - confidenceWeight(left.confidence) ||
    repairabilityWeight(right) - repairabilityWeight(left) ||
    numericScore(right.score_0_10000) - numericScore(left.score_0_10000) ||
    String(left.scope ?? '').localeCompare(String(right.scope ?? '')) ||
    String(left.kind ?? '').localeCompare(String(right.kind ?? ''))
  );
}

function sortPacketSamplesByPriority(samples) {
  return [...samples].sort(comparePacketSamples);
}

function packetSampleExpectedSummaryPresence(
  sample,
  orderedSamples,
  index,
  defaultPresence = 'section_present',
) {
  if (defaultPresence === 'side_channel') {
    return 'side_channel';
  }

  const sampleKindWeight = actionKindWeight(sample.kind ?? '');
  if (
    sampleKindWeight === 0 &&
    orderedSamples
      .slice(0, index)
      .some((peer) => actionKindWeight(peer.kind ?? '') > sampleKindWeight)
  ) {
    return 'side_channel';
  }

  if (index < 3) {
    return 'headline';
  }

  return defaultPresence === 'headline' ? 'section_present' : defaultPresence;
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

function buildVerdictIdentityFields(sample) {
  return {
    source_kind: sample.source_kind ?? null,
    source_label: sample.source_label ?? null,
    snapshot_label: sample.snapshot_label ?? null,
    task_id: sample.task_id ?? null,
    replay_id: sample.replay_id ?? null,
    commit: sample.commit ?? null,
  };
}

function resolveRepairSurface(expectedFixSurface, likelyFixSites) {
  if (hasText(expectedFixSurface)) {
    return expectedFixSurface;
  }
  if (likelyFixSites.length > 0) {
    return 'concrete_fix_site';
  }

  return null;
}

function resolveScanMetadataValue(payload, scanPayload, fieldName) {
  if (isPlainObject(payload?.[fieldName])) {
    return payload[fieldName];
  }
  if (isPlainObject(scanPayload?.[fieldName])) {
    return scanPayload[fieldName];
  }

  return null;
}

function isRepairPacketComplete(sample) {
  return sample.repair_packet?.complete === true;
}

function countCompleteRepairPackets(samples) {
  return samples.filter(isRepairPacketComplete).length;
}

function buildTemplateEngineerNote(sample) {
  const summary = sample.summary ?? 'Replace with reviewer rationale.';
  if (sample.repair_packet?.complete !== false) {
    return summary;
  }

  return `${summary} Confirm usefulness, rank, and whether missing repair guidance (${sample.repair_packet.missing_fields.join(', ')}) keeps this out of the primary surface.`;
}

function buildTemplateExpectedV2Behavior(sample, expectedSummaryPresence) {
  const summarySurface = summarySurfaceLabel(expectedSummaryPresence);
  if (sample.repair_packet?.complete !== false) {
    return `Confirm the ranking and presentation for ${sample.kind ?? 'this finding'} on the ${summarySurface}.`;
  }

  return `Confirm the ranking and presentation for ${sample.kind ?? 'this finding'} on the ${summarySurface}, and add explicit repair guidance before treating it as promotion-grade evidence.`;
}

function buildRepairPacket(sample, scope, summary, evidence, expectedFixSurface) {
  const likelyFixSites = buildLikelyFixSites(sample, scope);
  const fixHint = hasText(sample.fix_hint) ? sample.fix_hint : null;
  const inspectionFocus = uniqueStrings(sample.inspection_focus ?? []);
  const repairSurface = resolveRepairSurface(expectedFixSurface, likelyFixSites);
  const requiredFieldState = {
    scope: hasText(scope),
    summary: hasText(summary),
    evidence: evidence.length > 0,
    repair_surface: hasText(repairSurface),
  };
  const preferredFieldState = {
    fix_hint: hasText(fixHint),
    likely_fix_sites: likelyFixSites.length > 0,
  };
  const fixSurfaceClear = requiredFieldState.repair_surface;
  const verificationClear = evidence.length > 0 || inspectionFocus.length > 0;
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
    fix_surface_clear: fixSurfaceClear,
    verification_clear: verificationClear,
    fix_hint: fixHint,
    likely_fix_sites: likelyFixSites,
    inspection_focus: inspectionFocus,
    expected_fix_surface: repairSurface,
  };
}

function buildScanMetadata(payload, sourceKind, sourceLabel, snapshotLabel, scanPayload = null) {
  const confidence = resolveScanMetadataValue(payload, scanPayload, 'confidence');
  const scanTrust = resolveScanMetadataValue(payload, scanPayload, 'scan_trust');

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
  const reportBucket = tool === 'check' ? 'actions' : tool;

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
      report_bucket: reportBucket,
      scope,
      severity: sample.severity ?? null,
      trust_tier: sample.trust_tier ?? null,
      presentation_class: sample.presentation_class ?? null,
      leverage_class: sample.leverage_class ?? null,
      score_0_10000: typeof sample.score_0_10000 === 'number' ? sample.score_0_10000 : null,
      source: sample.source ?? null,
      origin: sample.origin ?? null,
      confidence: sample.confidence ?? null,
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

function packetSampleDeduplicationKey(sample) {
  return [
    sample.kind ?? '',
    sample.scope ?? '',
    sample.report_bucket ?? '',
    sample.source_kind ?? '',
    sample.source_label ?? '',
    sample.snapshot_label ?? '',
    sample.task_id ?? '',
    sample.replay_id ?? '',
    sample.commit ?? '',
    sample.output_dir ?? '',
  ].join('\u0000');
}

function packetSampleRepairCompleteness(sample) {
  return sample.repair_packet?.completeness_0_10000 ?? 0;
}

function packetSampleEvidenceCount(sample) {
  return Array.isArray(sample.evidence) ? sample.evidence.length : 0;
}

function packetSampleSummaryLength(sample) {
  return typeof sample.summary === 'string' ? sample.summary.length : 0;
}

function selectPreferredDuplicateSample(existingSample, nextSample) {
  if (
    packetSampleRepairCompleteness(nextSample) >
    packetSampleRepairCompleteness(existingSample)
  ) {
    return nextSample;
  }

  if (
    packetSampleRepairCompleteness(nextSample) <
    packetSampleRepairCompleteness(existingSample)
  ) {
    return existingSample;
  }

  if (packetSampleEvidenceCount(nextSample) > packetSampleEvidenceCount(existingSample)) {
    return nextSample;
  }

  if (packetSampleEvidenceCount(nextSample) < packetSampleEvidenceCount(existingSample)) {
    return existingSample;
  }

  if (packetSampleSummaryLength(nextSample) > packetSampleSummaryLength(existingSample)) {
    return nextSample;
  }

  return existingSample;
}

function dedupePacketSamples(samples) {
  const deduped = [];
  const sampleIndexByKey = new Map();

  for (const sample of samples) {
    const key = packetSampleDeduplicationKey(sample);
    const existingIndex = sampleIndexByKey.get(key);

    if (existingIndex === undefined) {
      sampleIndexByKey.set(key, deduped.length);
      deduped.push(sample);
      continue;
    }

    deduped[existingIndex] = selectPreferredDuplicateSample(deduped[existingIndex], sample);
  }

  return deduped;
}

function collectArtifactMetadata(tool, entries) {
  for (const entry of entries) {
    const bundleMetadata = buildScanMetadata(
      entry.bundle,
      entry.source_kind,
      entry.source_label,
      'bundle',
    );
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

function buildPacketSamplesFromEntries(tool, entries, kinds) {
  const samples = [];
  const metadataSelectionBySampleKey = new Map();

  for (const entry of entries) {
    const payloadEntry = selectArtifactPayload(tool, entry.bundle);
    if (!payloadEntry) {
      continue;
    }

    const entrySamples = filterSamplesByKinds(extractSamples(tool, payloadEntry, entry), kinds);

    for (const sample of entrySamples) {
      const sampleKey = packetSampleDeduplicationKey(sample);
      if (!metadataSelectionBySampleKey.has(sampleKey)) {
        metadataSelectionBySampleKey.set(sampleKey, {
          entry,
          payloadEntry,
        });
      }
      samples.push(sample);
    }
  }

  return {
    samples,
    metadataSelectionBySampleKey,
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
  const repairPacketCompleteCount = countCompleteRepairPackets(samples);
  const top3RepairPacketCompleteCount = countCompleteRepairPackets(top3Samples);
  const top10RepairPacketCompleteCount = countCompleteRepairPackets(top10Samples);

  for (const sample of samples) {
    const key = sample.kind ?? 'unknown';
    kindCounts.set(key, (kindCounts.get(key) ?? 0) + 1);
  }

  return {
    sample_count: samples.length,
    repair_packet_complete_count: repairPacketCompleteCount,
    repair_packet_complete_rate: samples.length
      ? repairPacketCompleteCount / samples.length
      : null,
    top_3_repair_packet_complete_rate: top3Samples.length
      ? top3RepairPacketCompleteCount / top3Samples.length
      : null,
    top_10_repair_packet_complete_rate: top10Samples.length
      ? top10RepairPacketCompleteCount / top10Samples.length
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

function selectPacketMetadataSelection(samples, metadataSelectionBySampleKey) {
  if (!Array.isArray(samples) || samples.length === 0) {
    return null;
  }

  const firstSample = samples[0];
  const firstSampleKey = packetSampleDeduplicationKey(firstSample);
  return metadataSelectionBySampleKey.get(firstSampleKey) ?? null;
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

export function buildPacketFromArtifactInput(args, source) {
  const repoRootValue = source.repo_root ?? args.repoRoot;
  const sampleSelection = buildPacketSamplesFromEntries(args.tool, source.entries, args.kinds);
  const dedupedSamples = dedupePacketSamples(sampleSelection.samples);
  const prioritizedSamples = sortPacketSamplesByPriority(dedupedSamples);
  const selectedSamples = limitSamples(prioritizedSamples, args.limit);
  const metadataSelection = selectPacketMetadataSelection(
    selectedSamples,
    sampleSelection.metadataSelectionBySampleKey,
  );
  const scanMetadata =
    collectSelectedArtifactMetadata(metadataSelection) ??
    collectArtifactMetadata(args.tool, source.entries);
  const samples = renumberSamples(args.tool, selectedSamples);

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
  const extractedSamples = extractSamples(
    args.tool,
    { snapshot_label: 'repo_head', payload },
    repoHeadEntry,
  );
  const filteredSamples = filterSamplesByKinds(extractedSamples, args.kinds);
  const dedupedSamples = dedupePacketSamples(filteredSamples);
  const prioritizedSamples = sortPacketSamplesByPriority(dedupedSamples);
  const selectedSamples = limitSamples(prioritizedSamples, args.limit);
  const samples = renumberSamples(args.tool, selectedSamples);

  return buildPacket(args, args.repoRoot, 'repo-head', [], samples, scanMetadata);
}

export function buildVerdictTemplate(packet, sourceReport) {
  const orderedSamples = sortPacketSamplesByPriority(packet.samples ?? []);
  const repo = packet.repo_root ? path.basename(packet.repo_root) : 'unknown';

  return {
    repo,
    captured_at: packet.generated_at,
    source_report: sourceReport,
    source_feedback:
      'Replace the placeholder verdict values below after reviewing the packet. Keep verdict order rank-preserving because top-1/top-3/top-10 actionable precision is computed from this order. Do not use this template as scored evidence until it has been curated by a reviewer.',
    verdicts: orderedSamples.map((sample, index) => {
      const expectedSummaryPresence = packetSampleExpectedSummaryPresence(
        sample,
        orderedSamples,
        index,
      );

      return {
        scope: sample.scope ?? sample.source_label ?? 'unknown-scope',
        kind: sample.kind ?? 'unknown-kind',
        report_bucket: sample.report_bucket,
        ...buildVerdictIdentityFields(sample),
        ...buildStructuredReviewVerdictFieldsFromPacketSample(sample, index),
        category: 'useful',
        expected_trust_tier: sample.severity === 'high' ? 'trusted' : 'watchpoint',
        expected_presentation_class: 'review_required',
        expected_leverage_class: sample.expected_fix_surface ?? 'local_refactor_target',
        expected_summary_presence: expectedSummaryPresence,
        preferred_over: [],
        engineer_note: buildTemplateEngineerNote(sample),
        expected_v2_behavior: buildTemplateExpectedV2Behavior(
          sample,
          expectedSummaryPresence,
        ),
      };
    }),
  };
}

export { comparePacketSamples, packetSampleExpectedSummaryPresence, sortPacketSamplesByPriority };
