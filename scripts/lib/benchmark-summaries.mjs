function countItems(value) {
  if (!Array.isArray(value)) {
    return null;
  }

  return value.length;
}

export function summarizeScan(payload) {
  return {
    files: payload.files,
    import_edges: payload.import_edges,
    quality_signal: payload.quality_signal,
    overall_confidence_0_10000: payload.scan_trust?.overall_confidence_0_10000 ?? null,
    resolved: payload.scan_trust?.resolution?.resolved ?? null,
    unresolved_internal: payload.scan_trust?.resolution?.unresolved_internal ?? null,
  };
}

export function summarizeConcepts(payload) {
  return {
    configured_concept_count: payload.summary?.configured_concept_count ?? null,
    contract_count: payload.summary?.contract_count ?? null,
    matched_guardrail_test_count: payload.summary?.matched_guardrail_test_count ?? null,
    inferred_concept_count: payload.summary?.inferred_concept_count ?? null,
    state_model_count: payload.summary?.state_model_count ?? null,
    semantic_cache_source: payload.semantic_cache?.source ?? null,
  };
}

export function summarizeFindings(payload) {
  return {
    clone_group_count: payload.clone_group_count ?? null,
    clone_family_count: payload.clone_family_count ?? null,
    finding_count: countItems(payload.findings),
    semantic_finding_count: payload.semantic_finding_count ?? null,
    debt_signal_count: payload.debt_signal_count ?? null,
    watchpoint_count: payload.watchpoint_count ?? null,
  };
}

export function summarizeExplainConcept(payload) {
  return {
    finding_count: countItems(payload.findings),
    obligation_count: countItems(payload.obligations),
    read_count: countItems(payload.semantic?.reads),
    write_count: countItems(payload.semantic?.writes),
    related_test_count: countItems(payload.related_tests),
  };
}

export function summarizeObligations(payload) {
  return {
    obligation_count: payload.obligation_count ?? null,
    missing_site_count: payload.missing_site_count ?? null,
    obligation_completeness_0_10000: payload.obligation_completeness_0_10000 ?? null,
  };
}

export function summarizeParity(payload) {
  return {
    contract_count: payload.contract_count ?? null,
    missing_cell_count: payload.missing_cell_count ?? null,
    parity_score_0_10000: payload.parity_score_0_10000 ?? null,
    finding_count: countItems(payload.findings),
  };
}

export function summarizeState(payload) {
  return {
    state_model_count: payload.state_model_count ?? null,
    finding_count: payload.finding_count ?? null,
    state_integrity_score_0_10000: payload.state_integrity_score_0_10000 ?? null,
  };
}

export function summarizeAgentBrief(payload) {
  return {
    mode: payload.mode ?? null,
    decision: payload.decision ?? null,
    primary_target_count: payload.primary_target_count ?? null,
    missing_obligation_count: payload.missing_obligation_count ?? null,
    watchpoint_count: payload.watchpoint_count ?? null,
    semantic_cache_source: payload.semantic_cache?.source ?? null,
  };
}

export function summarizeGate(payload) {
  return {
    decision: payload.decision ?? null,
    changed_file_count: countItems(payload.changed_files),
    introduced_finding_count: countItems(payload.introduced_findings),
    missing_obligation_count: countItems(payload.missing_obligations),
    obligation_completeness_0_10000: payload.obligation_completeness_0_10000 ?? null,
  };
}

export function summarizeSessionSave(payload) {
  return {
    session_finding_count: payload.session_finding_count ?? null,
    suppressed_finding_count: payload.suppressed_finding_count ?? null,
  };
}

export function summarizeSessionEnd(payload) {
  return {
    pass: payload.pass ?? null,
    changed_file_count: countItems(payload.changed_files),
    introduced_finding_count: countItems(payload.introduced_findings),
    missing_obligation_count: countItems(payload.missing_obligations),
    gate_decision: payload.touched_concept_gate?.decision ?? null,
  };
}

export function summarizeCheck(payload) {
  return {
    gate: payload.gate ?? null,
    issue_count: countItems(payload.issues),
    changed_file_count: countItems(payload.changed_files),
    partial_results: payload.diagnostics?.partial_results ?? null,
    changed_scope_available: payload.diagnostics?.availability?.changed_scope ?? null,
  };
}

export function summarizeProjectShape(payload) {
  const projectShape = payload.project_shape;

  return {
    primary_archetype: projectShape?.primary_archetype ?? null,
    effective_archetype_count: countItems(projectShape?.effective_archetypes),
    capability_count: countItems(projectShape?.capabilities),
    boundary_root_count: countItems(projectShape?.boundary_roots),
    module_contract_count: countItems(projectShape?.module_contracts),
  };
}

export function summarizeCheckRules(payload) {
  return {
    pass: payload.pass ?? null,
    rules_checked: payload.rules_checked ?? null,
    violation_count: payload.violation_count ?? null,
    coverage_0_10000: payload.v2_rule_coverage?.coverage_0_10000 ?? null,
    truncated: payload.truncated?.rules_checked ?? null,
  };
}
