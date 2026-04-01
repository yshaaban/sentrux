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

fn finding_detail_impact(finding: &Value) -> String {
    if let Some(impact) = finding.get("impact").and_then(|value| value.as_str()) {
        return impact.to_string();
    }

    match finding_kind(finding) {
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
        "contract_surface_completeness" => {
            "Related contract surfaces are no longer aligned, so runtime paths can diverge or partially break.".to_string()
        }
        "state_integrity_missing_site" | "state_integrity_unmapped_root" => {
            "State model drift makes lifecycle and restore behavior easier to break through partial edits.".to_string()
        }
        "contract_parity_gap" => {
            "Cross-surface parity drift means different runtime paths may no longer implement the same contract.".to_string()
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

    let focus = match finding_kind(finding) {
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
        "closed_domain_exhaustiveness" | "contract_surface_completeness" => vec![
            "inspect which required surfaces should change together and add explicit coverage there"
                .to_string(),
        ],
        "exact_clone_group" | "clone_family" => clone_family_inspection_focus(finding),
        _ => vec![
            "inspect the files and evidence behind this finding before choosing a design fix"
                .to_string(),
        ],
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
    let mut merged: Vec<(u8, Value)> = other_findings
        .into_iter()
        .map(|finding| (severity_priority(severity_of_value(&finding)), finding))
        .collect();
    merged.extend(
        clone_findings
            .into_iter()
            .map(|finding| (severity_priority(severity_of_value(&finding)), finding)),
    );
    merged.sort_by(|left, right| right.0.cmp(&left.0));
    merged
        .into_iter()
        .take(limit)
        .map(|(_, finding)| finding)
        .collect()
}

pub(crate) fn severity_priority(severity: FindingSeverity) -> u8 {
    match severity {
        FindingSeverity::High => 3,
        FindingSeverity::Medium => 2,
        FindingSeverity::Low => 1,
    }
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
                json!({"kind": "contract_surface_completeness", "severity": "medium"}),
            ],
            3,
        );

        assert_eq!(merged[0]["severity"], "high");
        assert_eq!(merged[1]["severity"], "medium");
        assert_eq!(merged[2]["severity"], "low");
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
