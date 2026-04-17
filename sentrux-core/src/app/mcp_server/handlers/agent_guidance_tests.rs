use super::*;
use serde_json::json;

#[test]
fn repair_packet_for_large_file_uses_shared_family_guidance() {
    let finding = json!({
        "files": ["src/app/mcp_server/handlers/agent_guidance.rs"],
        "candidate_split_axes": ["packet assembly"],
        "related_surfaces": ["src/app/mcp_server/handlers/agent_format.rs"],
        "kind": "large_file",
        "evidence": ["file scale signal"],
    });

    let packet = repair_packet_for_finding(&finding, "large_file");

    assert!(packet.risk_statement.contains("change friction"));
    assert!(packet
        .risk_statement
        .contains("future regressions harder to isolate"));
    assert_eq!(
        packet.smallest_safe_first_cut.as_deref(),
        Some("Split the file along the packet assembly and move the behavior that couples to src/app/mcp_server/handlers/agent_format.rs behind a smaller owner before adding more code here.")
    );
    assert_eq!(
        packet.do_not_touch_yet,
        vec!["Do not slice the file by line count alone; split along the cited boundary instead."]
    );
    assert_eq!(packet.verify_after.len(), 1);
    assert!(
        packet.verify_after[0].contains("agent_guidance.rs")
            || packet.verify_after[0].contains("src/app/mcp_server/handlers/agent_format.rs")
    );
}

#[test]
fn repair_packet_for_clone_propagation_drift_uses_family_specific_hint() {
    let finding = json!({
        "files": ["src/app/mcp_server/handlers/agent_guidance.rs", "src/app/mcp_server/handlers/agent_format.rs"],
        "evidence": [
            "changed clone member: src/app/mcp_server/handlers/agent_guidance.rs",
            "unchanged clone sibling: src/app/mcp_server/handlers/agent_format.rs"
        ],
        "kind": "clone_propagation_drift"
    });

    let packet = repair_packet_for_finding(&finding, "clone_propagation_drift");

    assert_eq!(
        packet.smallest_safe_first_cut.as_deref(),
        Some("Sync src/app/mcp_server/handlers/agent_format.rs with the behavior change in src/app/mcp_server/handlers/agent_guidance.rs, or collapse both paths behind one shared owner.")
    );
    assert_eq!(
        packet.do_not_touch_yet,
        vec!["Do not create a third sibling path while fixing the duplicate behavior."]
    );
}

#[test]
fn repair_packet_for_closed_domain_obligation_keeps_gate_verification() {
    let obligation = json!({
        "kind": "closed_domain_exhaustiveness",
        "concept_id": "signal_kind",
        "domain_symbol_name": "signal kind",
        "missing_variants": ["alpha", "beta"],
        "missing_sites": [
            {"path": "src/app/mcp_server/handlers/agent_guidance.rs", "line": 42, "detail": "missing exhaustive branch"}
        ]
    });

    let packet = repair_packet_for_obligation(&obligation, "closed_domain_exhaustiveness");

    assert_eq!(
        packet.risk_statement,
        "Finite-domain changes can silently miss one surface unless every required branch stays in sync."
    );
    assert!(packet
        .smallest_safe_first_cut
        .as_deref()
        .unwrap_or_default()
        .contains("Handle the missing variants [alpha, beta]"));
    assert!(packet
        .verify_after
        .iter()
        .any(|step| step.contains("Re-run `sentrux gate`")));
    assert_eq!(
        packet.do_not_touch_yet,
        vec!["Do not rely on a production fallback or default branch to hide missing variants."]
    );
}

#[test]
fn repair_packet_for_raw_read_keeps_accessor_based_hint() {
    let finding = json!({
        "files": ["src/app/mcp_server/handlers/agent_guidance.rs"],
        "evidence": [
            "preferred accessor: use_signal_summary",
            "canonical owner: SignalSummary"
        ],
        "kind": "forbidden_raw_read"
    });

    let packet = repair_packet_for_finding(&finding, "forbidden_raw_read");

    assert_eq!(
        packet.smallest_safe_first_cut.as_deref(),
        Some("Replace the raw read with use_signal_summary from SignalSummary instead of recreating the projection in the caller.")
    );
    assert!(packet
        .verify_after
        .iter()
        .any(|step| step.contains("agent_guidance.rs")));
}
