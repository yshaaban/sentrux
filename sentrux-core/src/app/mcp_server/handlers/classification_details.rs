use super::*;
use crate::metrics::v2::FindingSeverity;

pub(crate) fn decorate_finding_with_classification(finding: &Value) -> Value {
    let presentation_class = finding_presentation_class(finding);
    let leverage_class = finding_leverage_class(finding);
    let leverage_reasons = finding_leverage_reasons(finding);
    let mut finding = finding.clone();
    if let Some(payload) = finding.as_object_mut() {
        payload.insert(
            "presentation_class".to_string(),
            Value::String(presentation_class.as_str().to_string()),
        );
        payload.insert(
            "leverage_class".to_string(),
            Value::String(leverage_class.as_str().to_string()),
        );
        payload.insert("leverage_reasons".to_string(), json!(leverage_reasons));
    }

    finding
}

pub(crate) fn is_experimental_finding(finding: &Value) -> bool {
    finding_trust_tier(finding) == FindingTrustTier::Experimental
}

pub(crate) fn partition_experimental_findings(
    findings: &[Value],
    limit: usize,
) -> (Vec<Value>, Vec<Value>) {
    let mut visible = Vec::new();
    let mut experimental = Vec::new();

    for finding in findings {
        if is_experimental_finding(finding) {
            if experimental.len() < limit {
                experimental.push(finding.clone());
            }
            continue;
        }
        visible.push(finding.clone());
    }

    (visible, experimental)
}

fn is_redundant_review_surface_experimental_kind(kind: &str) -> bool {
    matches!(kind, "dead_private_code_cluster")
}

pub(crate) fn partition_review_surface_experimental_findings(
    findings: &[Value],
    limit: usize,
) -> (Vec<Value>, Vec<Value>) {
    let mut visible = Vec::new();
    let mut experimental = Vec::new();

    for finding in findings {
        if is_experimental_finding(finding) {
            if experimental.len() < limit
                && !is_redundant_review_surface_experimental_kind(finding_kind(finding))
            {
                experimental.push(finding.clone());
            }
            continue;
        }
        visible.push(finding.clone());
    }

    (visible, experimental)
}

pub(crate) fn build_finding_details(findings: &[Value], limit: usize) -> Vec<FindingDetail> {
    findings.iter().take(limit).map(finding_detail).collect()
}

fn finding_detail(finding: &Value) -> FindingDetail {
    let kind = finding_kind(finding).to_string();
    let files = dedupe_strings_preserve_order(finding_files(finding));
    let evidence = dedupe_strings_preserve_order(finding_string_values(finding, "evidence"));
    let inspection_focus = finding_detail_inspection_focus(finding);

    annotate_finding_detail(FindingDetail {
        kind: kind.clone(),
        trust_tier: finding_trust_tier(finding),
        presentation_class: finding_presentation_class(finding),
        leverage_class: finding_leverage_class(finding),
        leverage_class_explicit: finding.get("leverage_class").is_some(),
        scope: finding_scope(finding),
        severity: severity_of_value(finding),
        summary: finding
            .get("summary")
            .and_then(|value| value.as_str())
            .unwrap_or(kind.as_str())
            .to_string(),
        impact: finding_detail_impact(finding),
        files: files.clone(),
        role_tags: finding_string_values(finding, "role_tags"),
        leverage_reasons: finding_leverage_reasons(finding),
        evidence: evidence.clone(),
        inspection_focus,
        candidate_split_axes: finding_detail_candidate_split_axes(finding),
        related_surfaces: finding_detail_related_surfaces(finding),
        metrics: FindingDetailMetrics {
            file_count: files.len(),
            evidence_count: evidence.len(),
            member_count: finding
                .get("member_count")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
            family_score_0_10000: finding
                .get("family_score")
                .or_else(|| finding.get("family_score_0_10000"))
                .and_then(|value| value.as_u64())
                .map(|value| value as u32),
            divergence_score: finding
                .get("divergence_score")
                .and_then(|value| value.as_u64())
                .map(|value| value as u32),
        },
    })
}

fn finding_detail_candidate_split_axes(finding: &Value) -> Vec<String> {
    let axes = finding_string_values(finding, "candidate_split_axes");
    if !axes.is_empty() {
        return axes;
    }

    match finding_kind(finding) {
        "cycle_cluster" => vec!["contract boundary".to_string()],
        "dependency_sprawl" => vec!["dependency boundary".to_string()],
        "unstable_hotspot" => vec!["stable contract boundary".to_string()],
        _ => Vec::new(),
    }
}

fn finding_detail_related_surfaces(finding: &Value) -> Vec<String> {
    let related = finding_string_values(finding, "related_surfaces");
    if !related.is_empty() {
        return related;
    }

    finding_files(finding).into_iter().take(5).collect()
}

fn clone_followthrough_inspection_focus() -> Vec<String> {
    vec![
        "inspect whether the changed path and the sibling clone should collapse behind one shared helper"
            .to_string(),
        "inspect whether the unchanged sibling still needs the same behavior update".to_string(),
    ]
}

fn finding_detail_impact(finding: &Value) -> String {
    if let Some(impact) = finding.get("impact").and_then(|value| value.as_str()) {
        return impact.to_string();
    }

    let kind = finding_kind(finding);
    if super::is_contract_surface_propagation_kind(kind) {
        return "Related contract surfaces are no longer aligned, so runtime paths can diverge or partially break.".to_string();
    }

    match kind {
        "multi_writer_concept" => {
            "Multiple write paths make the concept easier to update inconsistently and harder to debug.".to_string()
        }
        "forbidden_writer" | "writer_outside_allowlist" => {
            "Writes from the wrong layer erode ownership and increase the chance that invariants drift.".to_string()
        }
        "forbidden_raw_read" | "authoritative_import_bypass" => {
            "Bypassing the intended read boundary weakens architectural contracts and can create stale or inconsistent views.".to_string()
        }
        "concept_boundary_pressure" => {
            "Repeated boundary bypasses around the same concept make future changes easier to scatter across the codebase.".to_string()
        }
        "closed_domain_exhaustiveness" => {
            "Finite-domain changes can silently miss one surface unless all required cases stay in sync.".to_string()
        }
        "state_integrity_missing_site" | "state_integrity_unmapped_root" => {
            "State model drift makes lifecycle and restore behavior easier to break through partial edits.".to_string()
        }
        "contract_parity_gap" => {
            "Cross-surface parity drift means different runtime paths may no longer implement the same contract.".to_string()
        }
        "session_introduced_clone" => {
            "Fresh duplication introduced in the current patch is likely to drift unless the new path is folded back into the original owner now.".to_string()
        }
        "clone_propagation_drift" => {
            "Changing one member of a clone family without syncing its sibling makes the next behavior change easier to miss.".to_string()
        }
        "touched_clone_family" => {
            "Touched clone families increase the chance that a sibling path quietly diverges on the next edit.".to_string()
        }
        "exact_clone_group" | "clone_family" => {
            "Duplicate logic increases the chance that fixes land in one copy but not the others.".to_string()
        }
        _ => "If ignored, this structural inconsistency will keep adding change friction and make future regressions harder to isolate.".to_string(),
    }
}

fn finding_detail_inspection_focus(finding: &Value) -> Vec<String> {
    let focus = finding_string_values(finding, "inspection_focus");
    if !focus.is_empty() {
        return focus;
    }

    let kind = finding_kind(finding);
    let focus = if kind == "closed_domain_exhaustiveness"
        || super::is_contract_surface_propagation_kind(kind)
    {
        vec![
            "inspect which required surfaces should change together and add explicit coverage there"
                .to_string(),
        ]
    } else {
        match kind {
            "multi_writer_concept" | "forbidden_writer" | "writer_outside_allowlist" => vec![
                "inspect which module should own writes for this concept".to_string(),
                "inspect whether the extra write path can be removed or routed through the owner"
                    .to_string(),
            ],
            "forbidden_raw_read" | "authoritative_import_bypass" | "concept_boundary_pressure" => {
                vec![
                "inspect whether reads should move behind the canonical accessor or public boundary"
                    .to_string(),
            ]
            }
            "session_introduced_clone" | "clone_propagation_drift" | "touched_clone_family" => {
                clone_followthrough_inspection_focus()
            }
            "exact_clone_group" | "clone_family" => clone_family_inspection_focus(finding),
            _ => vec![
                "inspect the files and evidence behind this finding before choosing a design fix"
                    .to_string(),
            ],
        }
    };

    let mut focus = dedupe_strings_preserve_order(focus);
    focus.truncate(3);
    focus
}

pub(crate) fn severity_of_value(value: &Value) -> FindingSeverity {
    match value.get("severity").and_then(|severity| severity.as_str()) {
        Some("high") => FindingSeverity::High,
        Some("medium") => FindingSeverity::Medium,
        _ => FindingSeverity::Low,
    }
}

pub(crate) fn merge_findings(
    clone_findings: Vec<Value>,
    other_findings: Vec<Value>,
    limit: usize,
) -> Vec<Value> {
    let mut merged = clone_findings;
    merged.extend(other_findings);
    merged.sort_by(compare_findings_for_brief);
    merged.into_iter().take(limit).collect()
}

pub(crate) fn severity_priority(severity: FindingSeverity) -> u8 {
    match severity {
        FindingSeverity::High => 3,
        FindingSeverity::Medium => 2,
        FindingSeverity::Low => 1,
    }
}

fn trust_tier_priority(trust_tier: FindingTrustTier) -> u8 {
    match trust_tier {
        FindingTrustTier::Trusted => 3,
        FindingTrustTier::Watchpoint => 2,
        FindingTrustTier::Experimental => 1,
    }
}

fn leverage_priority(leverage_class: FindingLeverageClass) -> u8 {
    (FindingLeverageClass::Experimental.rank() - leverage_class.rank()) as u8
}

fn presentation_priority(presentation_class: FindingPresentationClass) -> u8 {
    (FindingPresentationClass::Experimental.rank() - presentation_class.rank()) as u8
}

fn finding_score_0_10000(finding: &Value) -> u64 {
    finding
        .get("score_0_10000")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn compare_findings_for_brief(left: &Value, right: &Value) -> std::cmp::Ordering {
    let left_severity = severity_priority(severity_of_value(left));
    let right_severity = severity_priority(severity_of_value(right));
    let left_leverage = leverage_priority(finding_leverage_class(left));
    let right_leverage = leverage_priority(finding_leverage_class(right));
    let left_presentation = presentation_priority(finding_presentation_class(left));
    let right_presentation = presentation_priority(finding_presentation_class(right));
    let left_trust = trust_tier_priority(finding_trust_tier(left));
    let right_trust = trust_tier_priority(finding_trust_tier(right));
    let left_score = finding_score_0_10000(left);
    let right_score = finding_score_0_10000(right);

    right_severity
        .cmp(&left_severity)
        .then_with(|| right_leverage.cmp(&left_leverage))
        .then_with(|| right_presentation.cmp(&left_presentation))
        .then_with(|| right_trust.cmp(&left_trust))
        .then_with(|| right_score.cmp(&left_score))
        .then_with(|| finding_scope(left).cmp(&finding_scope(right)))
        .then_with(|| finding_kind(left).cmp(finding_kind(right)))
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub(crate) struct FindingDetail {
    pub(crate) kind: String,
    pub(crate) trust_tier: FindingTrustTier,
    pub(crate) presentation_class: FindingPresentationClass,
    pub(crate) leverage_class: FindingLeverageClass,
    #[serde(skip)]
    pub(crate) leverage_class_explicit: bool,
    pub(crate) scope: String,
    pub(crate) severity: FindingSeverity,
    pub(crate) summary: String,
    pub(crate) impact: String,
    pub(crate) files: Vec<String>,
    pub(crate) role_tags: Vec<String>,
    pub(crate) leverage_reasons: Vec<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) inspection_focus: Vec<String>,
    pub(crate) candidate_split_axes: Vec<String>,
    pub(crate) related_surfaces: Vec<String>,
    pub(crate) metrics: FindingDetailMetrics,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub(crate) struct FindingDetailMetrics {
    pub(crate) file_count: usize,
    pub(crate) evidence_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) member_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) family_score_0_10000: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) divergence_score: Option<u32>,
}

pub(crate) fn annotate_finding_detail(mut detail: FindingDetail) -> FindingDetail {
    let fan_in = detail
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix("fan-in: "))
        .and_then(|value| value.parse::<usize>().ok());
    let fan_out = detail
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix("fan-out: "))
        .and_then(|value| value.parse::<usize>().ok());
    let cycle_size = detail
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix("cycle size: "))
        .and_then(|value| value.parse::<usize>().ok());
    let cut_candidate_count = detail
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix("candidate cuts: "))
        .and_then(|value| value.parse::<usize>().ok());
    if !detail.leverage_class_explicit && detail.leverage_class == FindingLeverageClass::default() {
        detail.leverage_class = classify_leverage_class_internal(
            &detail.kind,
            detail.trust_tier,
            detail.presentation_class,
            &detail.role_tags,
            fan_in,
            fan_out,
            None,
            None,
            cycle_size,
            cut_candidate_count,
            None,
            0,
            0,
        );
    }

    if detail.leverage_reasons.is_empty() {
        detail.leverage_reasons = classify_leverage_reasons_internal(
            &detail.kind,
            detail.trust_tier,
            detail.presentation_class,
            detail.leverage_class,
            &detail.role_tags,
            fan_in,
            fan_out,
            cycle_size,
            cut_candidate_count,
            None,
            0,
            0,
        );
    }
    detail
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_findings_orders_by_typed_severity_priority() {
        let merged = merge_findings(
            vec![json!({"kind": "clone_family", "severity": "low"})],
            vec![
                json!({"kind": "multi_writer_concept", "severity": "high"}),
                json!({"kind": "incomplete_propagation", "severity": "medium"}),
            ],
            3,
        );

        assert_eq!(merged[0]["severity"], "high");
        assert_eq!(merged[1]["severity"], "medium");
        assert_eq!(merged[2]["severity"], "low");
    }

    #[test]
    fn merge_findings_prefers_architecture_signals_over_clone_cleanup_at_equal_severity() {
        let merged = merge_findings(
            vec![json!({
                "kind": "exact_clone_group",
                "severity": "high",
                "summary": "clone cleanup",
                "files": ["src/a.ts", "src/b.ts"],
            })],
            vec![json!({
                "kind": "dependency_sprawl",
                "severity": "high",
                "summary": "dependency sprawl",
                "scope": "src/App.tsx",
                "role_tags": ["composition_root", "guarded_seam"],
                "evidence": ["fan-out: 37"],
                "score_0_10000": 8200,
            })],
            2,
        );

        assert_eq!(merged[0]["kind"], "dependency_sprawl");
        assert_eq!(merged[1]["kind"], "exact_clone_group");
    }

    #[test]
    fn merge_findings_prefers_cycle_architecture_pressure_over_clone_cleanup() {
        let merged = merge_findings(
            vec![json!({
                "kind": "exact_clone_group",
                "severity": "high",
                "summary": "clone cleanup",
                "files": ["src/a.ts", "src/b.ts"],
            })],
            vec![json!({
                "kind": "cycle_cluster",
                "severity": "high",
                "summary": "store cycle",
                "files": ["src/store/core.ts", "src/store/store.ts"],
                "role_tags": [
                    "guarded_seam",
                    "state_container",
                    "guarded_boundary",
                    "component_barrel"
                ],
                "evidence": ["cycle size: 12", "candidate cuts: 3"],
                "score_0_10000": 9100,
            })],
            2,
        );

        assert_eq!(merged[0]["kind"], "cycle_cluster");
        assert_eq!(merged[1]["kind"], "exact_clone_group");
    }

    #[test]
    fn incomplete_propagation_reuses_contract_surface_detail_copy() {
        let detail = build_finding_details(
            &[json!({
                "kind": "incomplete_propagation",
                "summary": "Propagation is incomplete for 'server_state_bootstrap'.",
                "severity": "medium",
                "files": [
                    "src/domain/server-state-bootstrap.ts",
                    "src/app/server-state-bootstrap-registry.ts"
                ]
            })],
            1,
        );

        assert_eq!(
            detail[0].impact,
            "Related contract surfaces are no longer aligned, so runtime paths can diverge or partially break."
        );
        assert!(detail[0]
            .inspection_focus
            .iter()
            .any(|focus| focus.contains("required surfaces should change together")));
    }

    #[test]
    fn annotate_finding_detail_preserves_explicit_leverage_metadata() {
        let detail = annotate_finding_detail(FindingDetail {
            kind: "unstable_hotspot".to_string(),
            trust_tier: FindingTrustTier::Trusted,
            presentation_class: FindingPresentationClass::GuardedFacade,
            leverage_class: FindingLeverageClass::BoundaryDiscipline,
            leverage_class_explicit: true,
            scope: "src/lib/ipc.ts".to_string(),
            severity: FindingSeverity::Medium,
            summary: "Transport facade is under pressure".to_string(),
            impact: "Glue can absorb domain logic.".to_string(),
            files: vec!["src/lib/ipc.ts".to_string()],
            role_tags: vec!["transport_facade".to_string()],
            leverage_reasons: vec!["boundary_or_facade_seam_pressure".to_string()],
            evidence: vec!["fan-in: 42".to_string()],
            inspection_focus: vec!["inspect policy leakage".to_string()],
            candidate_split_axes: vec!["transport boundary".to_string()],
            related_surfaces: vec!["src/lib/ipc.ts".to_string()],
            metrics: FindingDetailMetrics::default(),
        });

        assert_eq!(detail.trust_tier, FindingTrustTier::Trusted);
        assert_eq!(
            detail.presentation_class,
            FindingPresentationClass::GuardedFacade
        );
        assert_eq!(
            detail.leverage_class,
            FindingLeverageClass::BoundaryDiscipline
        );
        assert_eq!(detail.severity, FindingSeverity::Medium);
        assert_eq!(
            detail.leverage_reasons,
            vec!["boundary_or_facade_seam_pressure".to_string()]
        );

        let serialized = serde_json::to_value(&detail).expect("serialize detail");
        assert_eq!(serialized["trust_tier"], "trusted");
        assert_eq!(serialized["presentation_class"], "guarded_facade");
        assert_eq!(serialized["leverage_class"], "boundary_discipline");
        assert_eq!(serialized["severity"], "medium");
    }
}
