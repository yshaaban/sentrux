import { createHash } from 'node:crypto';

const BOUNDED_ADJUDICATION_MODEL_PROFILE = {
  provider_family: 'minimax',
  model: 'MiniMax M2.7',
  mode: 'structured_json',
  rationale:
    'Use MiniMax M2.7 only for bounded semantic adjudication over structured evidence bundles.',
};

const BOUNDED_ADJUDICATION_ALLOWED_VERDICTS = [
  'keep',
  'rerank_lower',
  'suppress',
  'needs_human_review',
];

const BOUNDED_ADJUDICATION_ALLOWED_RANKING_ACTIONS = [
  'hold_position',
  'lower_rank',
  'remove_from_default_lane',
  'leave_unapplied',
];

const BOUNDED_ADJUDICATION_OUTPUT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: [
    'task_kind',
    'bundle_id',
    'repo_name',
    'decision',
    'cited_evidence_ids',
    'confidence_0_1',
    'audit',
  ],
  properties: {
    task_kind: { const: 'bounded_adjudication' },
    bundle_id: { type: 'string', minLength: 1 },
    repo_name: { type: 'string', minLength: 1 },
    decision: {
      type: 'object',
      additionalProperties: false,
      required: ['verdict', 'ranking_action', 'summary'],
      properties: {
        verdict: {
          enum: BOUNDED_ADJUDICATION_ALLOWED_VERDICTS,
        },
        ranking_action: {
          enum: BOUNDED_ADJUDICATION_ALLOWED_RANKING_ACTIONS,
        },
        summary: { type: 'string', minLength: 1 },
        rationale: { type: 'string' },
      },
    },
    cited_evidence_ids: {
      type: 'array',
      minItems: 1,
      uniqueItems: true,
      items: { type: 'string', minLength: 1 },
    },
    cited_fix_site_ids: {
      type: 'array',
      uniqueItems: true,
      items: { type: 'string', minLength: 1 },
    },
    cited_verification_surface_ids: {
      type: 'array',
      uniqueItems: true,
      items: { type: 'string', minLength: 1 },
    },
    confidence_0_1: {
      type: 'number',
      minimum: 0,
      maximum: 1,
    },
    evidence_gaps: {
      type: 'array',
      items: { type: 'string', minLength: 1 },
    },
    notes: {
      type: 'array',
      items: { type: 'string', minLength: 1 },
    },
    audit: {
      type: 'object',
      additionalProperties: false,
      required: ['structured_evidence_only', 'requires_human_review', 'auto_apply_eligible'],
      properties: {
        structured_evidence_only: { const: true },
        requires_human_review: { type: 'boolean' },
        auto_apply_eligible: { const: false },
      },
    },
  },
};

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function nonEmptyStringOrNull(value) {
  return typeof value === 'string' && value.trim() ? value.trim() : null;
}

function uniqueStrings(values) {
  return [...new Set(asArray(values).map(nonEmptyStringOrNull).filter(Boolean))];
}

function normalizeEntityCollection(values, idPrefix, fieldMap = {}) {
  return asArray(values).map(function normalizeEntity(entry, index) {
    const source = entry && typeof entry === 'object' ? entry : {};
    const normalized = {
      id: nonEmptyStringOrNull(source.id) ?? `${idPrefix}${index + 1}`,
    };

    for (const [targetField, sourceField] of Object.entries(fieldMap)) {
      const sourceValue = source[sourceField];
      if (Array.isArray(sourceValue)) {
        normalized[targetField] = uniqueStrings(sourceValue);
      } else if (typeof sourceValue === 'number' && Number.isFinite(sourceValue)) {
        normalized[targetField] = sourceValue;
      } else {
        normalized[targetField] = nonEmptyStringOrNull(sourceValue);
      }
    }

    return normalized;
  });
}

function normalizeDiffSlice(diffSlice) {
  if (!diffSlice || typeof diffSlice !== 'object') {
    return null;
  }

  return {
    summary: nonEmptyStringOrNull(diffSlice.summary),
    files: uniqueStrings(diffSlice.files),
    changed_symbols: uniqueStrings(diffSlice.changed_symbols),
    hunks: asArray(diffSlice.hunks).map(function normalizeHunk(hunk, index) {
      const source = hunk && typeof hunk === 'object' ? hunk : {};
      return {
        id: nonEmptyStringOrNull(source.id) ?? `h${index + 1}`,
        path: nonEmptyStringOrNull(source.path),
        header: nonEmptyStringOrNull(source.header),
        summary: nonEmptyStringOrNull(source.summary),
      };
    }),
  };
}

function normalizePhaseTracking(phaseTracking) {
  if (!phaseTracking || typeof phaseTracking !== 'object') {
    return null;
  }

  return {
    phase_id: nonEmptyStringOrNull(phaseTracking.phase_id),
    status: nonEmptyStringOrNull(phaseTracking.status),
    milestone: nonEmptyStringOrNull(phaseTracking.milestone),
    note: nonEmptyStringOrNull(phaseTracking.note),
  };
}

function buildEvidenceBundleHash(bundle) {
  return createHash('sha256').update(JSON.stringify(bundle)).digest('hex');
}

function summarizeReferenceCoverage(citedIds, allowedIds) {
  const allowedSet = new Set(allowedIds);
  const normalizedCitedIds = uniqueStrings(citedIds);
  const invalid = normalizedCitedIds.filter(function isInvalid(entry) {
    return !allowedSet.has(entry);
  });

  return {
    cited_count: normalizedCitedIds.length,
    allowed_count: allowedSet.size,
    invalid_ids: invalid,
    all_cited_ids_valid: invalid.length === 0,
  };
}

function normalizeEvidenceBundle(task, scenario = null) {
  const sourceBundle =
    task?.evidence_bundle && typeof task.evidence_bundle === 'object' ? task.evidence_bundle : {};
  const repoName =
    nonEmptyStringOrNull(sourceBundle.repo_name) ??
    nonEmptyStringOrNull(scenario?.repo?.name) ??
    'unknown';
  const adjudicationTarget =
    sourceBundle.adjudication_target && typeof sourceBundle.adjudication_target === 'object'
      ? sourceBundle.adjudication_target
      : {};
  const evidenceItems = normalizeEntityCollection(sourceBundle.evidence_items, 'e', {
    kind: 'kind',
    summary: 'summary',
    source: 'source',
    path: 'path',
    line: 'line',
  });
  const candidateFixSites = normalizeEntityCollection(sourceBundle.candidate_fix_sites, 'f', {
    path: 'path',
    symbol: 'symbol',
    rationale: 'rationale',
  });
  const verificationSurfaces = normalizeEntityCollection(
    sourceBundle.verification_surfaces,
    'v',
    {
      kind: 'kind',
      path: 'path',
      command: 'command',
      rationale: 'rationale',
    },
  );

  const normalizedBundle = {
    schema_version: 1,
    bundle_kind: 'bounded_adjudication',
    bundle_id: nonEmptyStringOrNull(sourceBundle.bundle_id) ?? `${task?.task_id ?? 'bundle'}-bundle`,
    repo_name: repoName,
    adjudication_target: {
      finding_kind: nonEmptyStringOrNull(adjudicationTarget.finding_kind),
      summary: nonEmptyStringOrNull(adjudicationTarget.summary),
      current_rank:
        Number.isInteger(adjudicationTarget.current_rank) && adjudicationTarget.current_rank > 0
          ? adjudicationTarget.current_rank
          : null,
      current_lane: nonEmptyStringOrNull(adjudicationTarget.current_lane),
      severity: nonEmptyStringOrNull(adjudicationTarget.severity),
      confidence_0_1:
        typeof adjudicationTarget.confidence_0_1 === 'number'
          ? adjudicationTarget.confidence_0_1
          : null,
      expected_fix_surface: nonEmptyStringOrNull(adjudicationTarget.expected_fix_surface),
    },
    diff_slice: normalizeDiffSlice(sourceBundle.diff_slice),
    evidence_items: evidenceItems,
    dependent_surfaces: normalizeEntityCollection(sourceBundle.dependent_surfaces, 'd', {
      kind: 'kind',
      path: 'path',
      rationale: 'rationale',
    }),
    candidate_fix_sites: candidateFixSites,
    verification_surfaces: verificationSurfaces,
    source_artifacts: uniqueStrings(sourceBundle.source_artifacts),
    phase_tracking: normalizePhaseTracking(
      sourceBundle.phase_tracking ?? task?.phase_tracking ?? null,
    ),
    policy: {
      structured_evidence_only: true,
      allow_repo_scan: false,
      allow_new_finding_kinds: false,
      auto_apply_eligible: false,
      allowed_verdicts: BOUNDED_ADJUDICATION_ALLOWED_VERDICTS,
      allowed_ranking_actions: BOUNDED_ADJUDICATION_ALLOWED_RANKING_ACTIONS,
    },
  };
  normalizedBundle.bundle_sha256 = buildEvidenceBundleHash(normalizedBundle);
  return normalizedBundle;
}

function buildBoundedAdjudicationPrompt(task, scenario, scenarioPath) {
  const bundle = normalizeEvidenceBundle(task, scenario);
  const lines = [
    `Repository: ${bundle.repo_name}`,
    `Repository root: ${scenarioPath ? scenarioPath : 'not_applicable'}`,
    'Task kind: bounded_adjudication',
    `Model profile: ${BOUNDED_ADJUDICATION_MODEL_PROFILE.model}`,
    '',
    'Adjudicate exactly one Sentrux candidate using only the structured evidence bundle below.',
    'Do not scan the repository, invent new findings, or rely on unstated context.',
    'Prefer rerank_lower, suppress, or needs_human_review over optimistic keep decisions.',
    'The decision must remain advisory only; auto-apply is forbidden in this scaffold.',
    '',
    'Allowed verdicts:',
    ...BOUNDED_ADJUDICATION_ALLOWED_VERDICTS.map(function formatVerdict(verdict) {
      return `- ${verdict}`;
    }),
    '',
    'Allowed ranking actions:',
    ...BOUNDED_ADJUDICATION_ALLOWED_RANKING_ACTIONS.map(function formatAction(action) {
      return `- ${action}`;
    }),
    '',
    'Structured evidence bundle:',
    JSON.stringify(bundle, null, 2),
  ];

  if (nonEmptyStringOrNull(task?.prompt)) {
    lines.splice(4, 0, `Adjudication request: ${task.prompt.trim()}`, '');
  }

  return lines.join('\n');
}

function buildBoundedAdjudicationChecks(task, scenario = null) {
  const bundle = normalizeEvidenceBundle(task, scenario);
  const evidenceIds = bundle.evidence_items.map(function collectId(entry) {
    return entry.id;
  });
  const fixSiteIds = bundle.candidate_fix_sites.map(function collectId(entry) {
    return entry.id;
  });
  const verificationSurfaceIds = bundle.verification_surfaces.map(function collectId(entry) {
    return entry.id;
  });

  return [
    { kind: 'has', path: 'task_kind', severity: 'required' },
    { kind: 'equals', path: 'bundle_id', value: bundle.bundle_id, severity: 'required' },
    { kind: 'equals', path: 'repo_name', value: bundle.repo_name, severity: 'required' },
    {
      kind: 'enum',
      path: 'decision.verdict',
      allowed: BOUNDED_ADJUDICATION_ALLOWED_VERDICTS,
      severity: 'required',
    },
    {
      kind: 'enum',
      path: 'decision.ranking_action',
      allowed: BOUNDED_ADJUDICATION_ALLOWED_RANKING_ACTIONS,
      severity: 'required',
    },
    { kind: 'has', path: 'decision.summary', severity: 'required' },
    { kind: 'has', path: 'confidence_0_1', severity: 'required' },
    { kind: 'min_items', path: 'cited_evidence_ids', min: 1, severity: 'required' },
    {
      kind: 'all_items_in_set',
      path: 'cited_evidence_ids',
      allowed: evidenceIds,
      severity: 'required',
    },
    {
      kind: 'all_items_in_set',
      path: 'cited_fix_site_ids',
      allowed: fixSiteIds,
      severity: 'optional',
    },
    {
      kind: 'all_items_in_set',
      path: 'cited_verification_surface_ids',
      allowed: verificationSurfaceIds,
      severity: 'optional',
    },
    {
      kind: 'equals',
      path: 'audit.structured_evidence_only',
      value: true,
      severity: 'required',
    },
    {
      kind: 'has',
      path: 'audit.requires_human_review',
      severity: 'required',
    },
    {
      kind: 'equals',
      path: 'audit.auto_apply_eligible',
      value: false,
      severity: 'required',
    },
  ];
}

function buildBoundedAdjudicationArtifact(task, responseJson, scenario = null) {
  if (task?.kind !== 'bounded_adjudication') {
    return null;
  }

  const bundle = normalizeEvidenceBundle(task, scenario);
  const response = responseJson && typeof responseJson === 'object' ? responseJson : {};
  const evidenceCoverage = summarizeReferenceCoverage(
    response.cited_evidence_ids,
    bundle.evidence_items.map(function collectId(entry) {
      return entry.id;
    }),
  );
  const fixSiteCoverage = summarizeReferenceCoverage(
    response.cited_fix_site_ids,
    bundle.candidate_fix_sites.map(function collectId(entry) {
      return entry.id;
    }),
  );
  const verificationCoverage = summarizeReferenceCoverage(
    response.cited_verification_surface_ids,
    bundle.verification_surfaces.map(function collectId(entry) {
      return entry.id;
    }),
  );

  return {
    audit_contract_version: 1,
    model_profile: BOUNDED_ADJUDICATION_MODEL_PROFILE,
    evidence_bundle: bundle,
    decision:
      response.decision && typeof response.decision === 'object'
        ? {
            verdict: nonEmptyStringOrNull(response.decision.verdict),
            ranking_action: nonEmptyStringOrNull(response.decision.ranking_action),
            summary: nonEmptyStringOrNull(response.decision.summary),
            rationale: nonEmptyStringOrNull(response.decision.rationale),
            confidence_0_1:
              typeof response.confidence_0_1 === 'number' ? response.confidence_0_1 : null,
          }
        : null,
    reference_audit: {
      evidence: evidenceCoverage,
      fix_sites: fixSiteCoverage,
      verification_surfaces: verificationCoverage,
    },
    conservative_guardrails: {
      structured_evidence_only: response.audit?.structured_evidence_only === true,
      requires_human_review:
        typeof response.audit?.requires_human_review === 'boolean'
          ? response.audit.requires_human_review
          : null,
      auto_apply_eligible:
        typeof response.audit?.auto_apply_eligible === 'boolean'
          ? response.audit.auto_apply_eligible
          : null,
    },
    evidence_gaps: uniqueStrings(response.evidence_gaps),
    cited_evidence_ids: uniqueStrings(response.cited_evidence_ids),
    cited_fix_site_ids: uniqueStrings(response.cited_fix_site_ids),
    cited_verification_surface_ids: uniqueStrings(response.cited_verification_surface_ids),
    phase_tracking: bundle.phase_tracking,
  };
}

export {
  BOUNDED_ADJUDICATION_ALLOWED_RANKING_ACTIONS,
  BOUNDED_ADJUDICATION_ALLOWED_VERDICTS,
  BOUNDED_ADJUDICATION_MODEL_PROFILE,
  BOUNDED_ADJUDICATION_OUTPUT_SCHEMA,
  buildBoundedAdjudicationArtifact,
  buildBoundedAdjudicationChecks,
  buildBoundedAdjudicationPrompt,
  normalizeEvidenceBundle,
};
