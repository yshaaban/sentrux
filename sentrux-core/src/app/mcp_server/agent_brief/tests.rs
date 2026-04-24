use super::{build_agent_brief, AgentBriefInput, AgentBriefMode};
use serde_json::{json, Value};

fn behavior_parity_fixture() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../scripts/tests/fixtures/policy-parity/behavior-parity.json"
    )))
    .expect("behavior parity fixture")
}

fn mode_from_fixture(value: &str) -> AgentBriefMode {
    match value {
        "repo_onboarding" => AgentBriefMode::RepoOnboarding,
        "patch" => AgentBriefMode::Patch,
        "pre_merge" => AgentBriefMode::PreMerge,
        other => panic!("unsupported brief mode fixture: {other}"),
    }
}

#[test]
fn onboarding_brief_prioritizes_trusted_findings_and_starter_rules() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::RepoOnboarding,
        repo_shape: json!({
            "primary_archetype": "modular_nextjs_frontend",
            "effective_archetypes": ["modular_nextjs_frontend", "react_frontend"],
            "boundary_roots": [{ "root": "src/modules" }],
            "starter_rules_toml": "[[module_contract]]",
        }),
        findings: vec![json!({
            "kind": "dependency_sprawl",
            "scope": "api_endpoints_registry",
            "severity": "high",
            "summary": "Entry surface fans out across too many owners",
            "trust_tier": "trusted",
            "leverage_class": "boundary_discipline",
            "inspection_focus": ["inspect canonical access"],
            "likely_fix_sites": [{ "site": "src/hooks/use-api.ts" }],
            "concept_id": "api_endpoints_registry",
            "score_0_10000": 6500,
        })],
        experimental_findings: vec![json!({
            "kind": "dead_private_code_cluster",
            "scope": "src/unused.ts",
            "summary": "experimental dead code",
        })],
        missing_obligations: Vec::new(),
        watchpoints: vec![json!({
            "kind": "clone_family",
            "scope": "src/a.ts|src/b.ts",
            "summary": "watchpoint",
        })],
        resolved_findings: Vec::new(),
        changed_files: Vec::new(),
        changed_concepts: Vec::new(),
        decision: None,
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9000 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9000 }),
        freshness: json!({ "baseline_loaded": false }),
        strict: None,
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["kind"], "agent_brief");
    assert_eq!(brief["mode"], "repo_onboarding");
    assert_eq!(
        brief["repo_shape"]["primary_archetype"],
        "modular_nextjs_frontend"
    );
    assert_eq!(brief["primary_target_count"], 1);
    assert_eq!(
        brief["primary_targets"][0]["scope"],
        "api_endpoints_registry"
    );
    assert_eq!(
        brief["primary_targets"][0]["summary"],
        "Entry surface fans out across too many owners (high_signal)"
    );
    assert_eq!(
        brief["primary_targets"][0]["next_tools"][0]["tool"],
        "explain_concept"
    );
    assert_eq!(brief["primary_targets"][0]["severity"], "high");
    assert_eq!(brief["primary_targets"][0]["trust_tier"], "trusted");
    assert_eq!(
        brief["primary_targets"][0]["leverage_class"],
        "boundary_discipline"
    );
    assert_eq!(brief["do_not_chase_count"], 1);
    assert_eq!(brief["do_not_chase"][0]["reason"], "experimental_detector");
}

#[test]
fn patch_brief_prioritizes_missing_obligations_and_touched_concepts() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::Patch,
        repo_shape: json!({
            "primary_archetype": "react_frontend",
            "effective_archetypes": ["react_frontend"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![json!({
            "kind": "dependency_sprawl",
            "scope": "task_git_status",
            "severity": "high",
            "summary": "task_git_status fans out across too many owners",
            "trust_tier": "trusted",
            "leverage_class": "architecture_signal",
            "inspection_focus": ["inspect write ownership"],
            "likely_fix_sites": [{ "site": "src/store/core.ts" }],
            "concept_id": "task_git_status",
            "score_0_10000": 9000,
        })],
        experimental_findings: Vec::new(),
        missing_obligations: vec![json!({
            "concept_id": "task_git_status",
            "summary": "task_git_status still misses a canonical projection update",
            "missing_sites": [{ "site": "src/app/task-presentation.ts" }],
        })],
        watchpoints: vec![json!({
            "kind": "clone_family",
            "scope": "src/a.ts|src/b.ts",
            "summary": "non-blocking watchpoint",
        })],
        resolved_findings: Vec::new(),
        changed_files: vec!["src/store/core.ts".to_string()],
        changed_concepts: vec!["task_git_status".to_string()],
        decision: Some("fail".to_string()),
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9100 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9100 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: Some(false),
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["mode"], "patch");
    assert_eq!(brief["decision"], "fix_now");
    assert_eq!(brief["primary_targets"][0]["kind"], "missing_obligation");
    assert_eq!(brief["primary_targets"][0]["blocking"], json!(true));
    assert!(
        brief["primary_targets"][0]["repair_packet"]["completeness_0_10000"]
            .as_u64()
            .is_some_and(|score| score >= 8000)
    );
    assert!(brief["primary_targets"][1]["why_now"]
        .as_array()
        .expect("why now")
        .iter()
        .any(|value| value == "touched_concept"));
    assert_eq!(brief["watchpoint_count"], 1);
}

#[test]
fn onboarding_brief_requires_a_complete_repair_packet_for_non_blocking_targets() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::RepoOnboarding,
        repo_shape: json!({
            "primary_archetype": "react_frontend",
            "effective_archetypes": ["react_frontend"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![json!({
            "kind": "dead_private_code_cluster",
            "scope": "src/app.ts",
            "severity": "low",
            "summary": "Private helpers are no longer referenced from live callers",
            "trust_tier": "trusted",
            "presentation_class": "structural_debt",
            "leverage_class": "secondary_cleanup",
            "score_0_10000": 9100,
            "files": ["src/app.ts"],
        })],
        experimental_findings: Vec::new(),
        missing_obligations: Vec::new(),
        watchpoints: Vec::new(),
        resolved_findings: Vec::new(),
        changed_files: Vec::new(),
        changed_concepts: Vec::new(),
        decision: None,
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9100 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9100 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: None,
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["primary_target_count"], 0);
}

#[test]
fn onboarding_brief_keeps_fixable_high_severity_structural_debt_visible() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::RepoOnboarding,
        repo_shape: json!({
            "primary_archetype": "rust_cli",
            "effective_archetypes": ["rust_cli"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![json!({
            "kind": "large_file",
            "scope": "src/main.rs",
            "severity": "high",
            "summary": "File 'src/main.rs' is 801 lines, above the rust threshold of 500",
            "trust_tier": "trusted",
            "presentation_class": "structural_debt",
            "leverage_class": "secondary_cleanup",
            "score_0_10000": 6895,
            "files": ["src/main.rs"],
            "candidate_split_axes": ["dependency boundary"],
            "related_surfaces": ["src/format.rs"],
        })],
        experimental_findings: Vec::new(),
        missing_obligations: Vec::new(),
        watchpoints: Vec::new(),
        resolved_findings: Vec::new(),
        changed_files: Vec::new(),
        changed_concepts: Vec::new(),
        decision: None,
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9000 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9000 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: None,
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["primary_target_count"], 1);
    assert_eq!(brief["primary_targets"][0]["kind"], "large_file");
    assert_eq!(brief["primary_targets"][0]["severity"], "high");
}

#[test]
fn onboarding_brief_demotes_raw_clone_groups_below_more_fixable_targets() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::RepoOnboarding,
        repo_shape: json!({
            "primary_archetype": "react_frontend",
            "effective_archetypes": ["react_frontend"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![
            json!({
                "kind": "exact_clone_group",
                "scope": "src/a.ts|src/b.ts",
                "severity": "high",
                "summary": "Exact clone group exists",
                "trust_tier": "watchpoint",
                "presentation_class": "watchpoint",
                "leverage_class": "secondary_cleanup",
                "score_0_10000": 9400,
                "files": ["src/a.ts", "src/b.ts"],
            }),
            json!({
                "kind": "dependency_sprawl",
                "scope": "src/app.ts",
                "severity": "high",
                "summary": "Entry surface fans out across too many owners",
                "trust_tier": "trusted",
                "presentation_class": "structural_debt",
                "leverage_class": "architecture_signal",
                "likely_fix_sites": [{ "site": "src/app.ts" }],
                "score_0_10000": 8200,
                "files": ["src/app.ts"],
            }),
        ],
        experimental_findings: Vec::new(),
        missing_obligations: Vec::new(),
        watchpoints: Vec::new(),
        resolved_findings: Vec::new(),
        changed_files: Vec::new(),
        changed_concepts: Vec::new(),
        decision: None,
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9100 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9100 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: None,
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["primary_targets"][0]["kind"], "dependency_sprawl");
    assert!(brief["primary_targets"]
        .as_array()
        .expect("primary targets")
        .iter()
        .all(|target| target["kind"] != "exact_clone_group"));
}

#[test]
fn onboarding_brief_applies_default_lane_primary_action_cap() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::RepoOnboarding,
        repo_shape: json!({
            "primary_archetype": "sdk",
            "effective_archetypes": ["sdk"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![
            json!({
                "kind": "authoritative_import_bypass",
                "scope": "api_client",
                "severity": "high",
                "summary": "api_client bypasses its canonical adapter",
                "trust_tier": "trusted",
                "presentation_class": "structural_debt",
                "leverage_class": "boundary_discipline",
                "likely_fix_sites": [{ "site": "src/api/client.ts" }],
                "score_0_10000": 9600,
                "files": ["src/api/client.ts"],
            }),
            json!({
                "kind": "forbidden_writer",
                "scope": "task_state",
                "severity": "high",
                "summary": "task_state is written outside the owner",
                "trust_tier": "trusted",
                "presentation_class": "structural_debt",
                "leverage_class": "boundary_discipline",
                "likely_fix_sites": [{ "site": "src/store/task-state.ts" }],
                "score_0_10000": 9500,
                "files": ["src/store/task-state.ts"],
            }),
            json!({
                "kind": "dependency_sprawl",
                "scope": "command_dispatch",
                "severity": "high",
                "summary": "command_dispatch fans out across too many owners",
                "trust_tier": "trusted",
                "presentation_class": "structural_debt",
                "leverage_class": "architecture_signal",
                "likely_fix_sites": [{ "site": "src/commands/dispatch.ts" }],
                "score_0_10000": 9400,
                "files": ["src/commands/dispatch.ts"],
            }),
            json!({
                "kind": "large_file",
                "scope": "src/components/task-panel.tsx",
                "severity": "high",
                "summary": "task-panel is over the size threshold",
                "trust_tier": "trusted",
                "presentation_class": "structural_debt",
                "leverage_class": "architecture_signal",
                "likely_fix_sites": [{ "site": "src/components/task-panel.tsx" }],
                "score_0_10000": 9300,
                "files": ["src/components/task-panel.tsx"],
            }),
        ],
        experimental_findings: Vec::new(),
        missing_obligations: Vec::new(),
        watchpoints: Vec::new(),
        resolved_findings: Vec::new(),
        changed_files: Vec::new(),
        changed_concepts: Vec::new(),
        decision: None,
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9300 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9300 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: None,
        limit: 10,
    })
    .expect("agent brief");

    assert_eq!(brief["primary_target_count"], 3);
}

#[test]
fn pre_merge_brief_does_not_reintroduce_policy_demoted_blockers() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::PreMerge,
        repo_shape: json!({
            "primary_archetype": "sdk",
            "effective_archetypes": ["sdk"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![json!({
            "kind": "governance_readiness",
            "scope": "release_gate",
            "severity": "high",
            "summary": "Release gate documentation is incomplete",
            "trust_tier": "trusted",
            "presentation_class": "structural_debt",
            "leverage_class": "architecture_signal",
            "likely_fix_sites": [{ "site": "docs/release.md" }],
            "score_0_10000": 9900,
            "files": ["docs/release.md"],
        })],
        experimental_findings: Vec::new(),
        missing_obligations: Vec::new(),
        watchpoints: Vec::new(),
        resolved_findings: Vec::new(),
        changed_files: vec!["docs/release.md".to_string()],
        changed_concepts: Vec::new(),
        decision: Some("fail".to_string()),
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9300 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9300 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: Some(true),
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["decision"], "block");
    assert_eq!(brief["primary_target_count"], 0);
}

#[test]
fn shared_behavior_fixtures_keep_representative_primary_targets_stable() {
    let fixture = behavior_parity_fixture();
    let cases = fixture["rust_brief_cases"]
        .as_array()
        .expect("rust brief cases");

    for case in cases {
        let brief = build_agent_brief(AgentBriefInput {
            mode: mode_from_fixture(case["mode"].as_str().expect("brief mode")),
            repo_shape: json!({
                "primary_archetype": "react_frontend",
                "effective_archetypes": ["react_frontend"],
                "boundary_roots": [],
                "starter_rules_toml": null,
            }),
            findings: case["findings"].as_array().expect("findings").to_vec(),
            experimental_findings: Vec::new(),
            missing_obligations: case["missing_obligations"]
                .as_array()
                .expect("missing obligations")
                .to_vec(),
            watchpoints: case["watchpoints"]
                .as_array()
                .expect("watchpoints")
                .to_vec(),
            resolved_findings: Vec::new(),
            changed_files: case["changed_files"]
                .as_array()
                .expect("changed files")
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            changed_concepts: case["changed_concepts"]
                .as_array()
                .expect("changed concepts")
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            decision: case["decision"].as_str().map(str::to_string),
            summary: None,
            confidence: json!({ "scan_confidence_0_10000": 9000 }),
            scan_trust: json!({ "overall_confidence_0_10000": 9000 }),
            freshness: json!({ "baseline_loaded": true }),
            strict: case["strict"].as_bool(),
            limit: case["limit"].as_u64().expect("limit") as usize,
        })
        .expect("agent brief");

        let expected_scopes = case["expected_primary_scopes"]
            .as_array()
            .expect("expected scopes")
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let expected_kinds = case["expected_primary_kinds"]
            .as_array()
            .expect("expected kinds")
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert_eq!(
            brief["primary_targets"]
                .as_array()
                .expect("primary targets")
                .iter()
                .map(|target| target["scope"].as_str().expect("target scope").to_string())
                .collect::<Vec<_>>(),
            expected_scopes,
            "{}",
            case["name"].as_str().expect("case name"),
        );
        assert_eq!(
            brief["primary_targets"]
                .as_array()
                .expect("primary targets")
                .iter()
                .map(|target| target["kind"].as_str().expect("target kind").to_string())
                .collect::<Vec<_>>(),
            expected_kinds,
            "{}",
            case["name"].as_str().expect("case name"),
        );
    }
}

#[test]
fn pre_merge_brief_blocks_on_gate_failures() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::PreMerge,
        repo_shape: json!({
            "primary_archetype": "sdk",
            "effective_archetypes": ["sdk"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![json!({
            "kind": "authoritative_import_bypass",
            "scope": "api_endpoints_registry",
            "severity": "high",
            "summary": "Canonical API path is bypassed",
            "trust_tier": "trusted",
            "leverage_class": "boundary_discipline",
            "inspection_focus": ["inspect canonical service access"],
            "likely_fix_sites": [{ "site": "src/services/api.ts" }],
            "concept_id": "api_endpoints_registry",
            "score_0_10000": 7000,
        })],
        experimental_findings: vec![json!({
            "kind": "dead_private_code_cluster",
            "scope": "src/unused.ts",
            "summary": "experimental",
        })],
        missing_obligations: Vec::new(),
        watchpoints: vec![json!({
            "kind": "clone_family",
            "scope": "src/a.ts|src/b.ts",
            "summary": "watchpoint",
        })],
        resolved_findings: Vec::new(),
        changed_files: vec!["src/services/api.ts".to_string()],
        changed_concepts: vec!["api_endpoints_registry".to_string()],
        decision: Some("fail".to_string()),
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 8800 }),
        scan_trust: json!({ "overall_confidence_0_10000": 8800 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: Some(true),
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["mode"], "pre_merge");
    assert_eq!(brief["decision"], "block");
    assert_eq!(
        brief["primary_targets"][0]["kind"],
        "authoritative_import_bypass"
    );
    assert!(brief["primary_targets"][0]["why_now"]
        .as_array()
        .expect("why now")
        .iter()
        .any(|value| value == "merge_blocker_candidate"));
    assert_eq!(brief["do_not_chase"][0]["reason"], "experimental_detector");
}

#[test]
fn pre_merge_brief_surfaces_medium_blockers_in_strict_mode() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::PreMerge,
        repo_shape: json!({
            "primary_archetype": "sdk",
            "effective_archetypes": ["sdk"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![json!({
            "kind": "authoritative_import_bypass",
            "scope": "api_endpoints_registry",
            "severity": "medium",
            "summary": "Canonical API path is bypassed",
            "trust_tier": "trusted",
            "leverage_class": "boundary_discipline",
            "inspection_focus": ["inspect canonical service access"],
            "likely_fix_sites": [{ "site": "src/services/api.ts" }],
            "concept_id": "api_endpoints_registry",
            "score_0_10000": 7000,
        })],
        experimental_findings: Vec::new(),
        missing_obligations: Vec::new(),
        watchpoints: Vec::new(),
        resolved_findings: Vec::new(),
        changed_files: vec!["src/services/api.ts".to_string()],
        changed_concepts: vec!["api_endpoints_registry".to_string()],
        decision: Some("fail".to_string()),
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 8800 }),
        scan_trust: json!({ "overall_confidence_0_10000": 8800 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: Some(true),
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["mode"], "pre_merge");
    assert_eq!(brief["decision"], "block");
    assert_eq!(brief["primary_target_count"], 1);
    assert_eq!(brief["primary_targets"][0]["severity"], "medium");
    assert_eq!(
        brief["primary_targets"][0]["scope"],
        "api_endpoints_registry"
    );
}

#[test]
fn pre_merge_brief_keeps_a_blocking_target_visible_when_scope_filtering_would_hide_it() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::PreMerge,
        repo_shape: json!({
            "primary_archetype": "sdk",
            "effective_archetypes": ["sdk"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![json!({
            "kind": "forbidden_writer",
            "scope": "task_state",
            "severity": "high",
            "summary": "task_state is being written outside the owner",
            "trust_tier": "trusted",
            "leverage_class": "boundary_discipline",
            "likely_fix_sites": [{ "site": "src/store/task-state.ts" }],
            "concept_id": "task_state",
            "score_0_10000": 9200,
            "files": ["src/store/task-state.ts"],
        })],
        experimental_findings: Vec::new(),
        missing_obligations: Vec::new(),
        watchpoints: Vec::new(),
        resolved_findings: Vec::new(),
        changed_files: vec!["src/components/task-row.tsx".to_string()],
        changed_concepts: Vec::new(),
        decision: Some("fail".to_string()),
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9200 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9200 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: Some(false),
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["decision"], "block");
    assert_eq!(brief["primary_target_count"], 1);
    assert_eq!(brief["primary_targets"][0]["blocking"], true);
    assert_eq!(brief["primary_targets"][0]["kind"], "forbidden_writer");
}

#[test]
fn pre_merge_brief_keeps_a_blocking_target_visible_when_limit_would_truncate_it() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::PreMerge,
        repo_shape: json!({
            "primary_archetype": "sdk",
            "effective_archetypes": ["sdk"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![
            json!({
                "kind": "large_file",
                "scope": "src/components/task-row.tsx",
                "severity": "high",
                "summary": "task-row is over the size threshold",
                "trust_tier": "trusted",
                "leverage_class": "architecture_signal",
                "likely_fix_sites": [{ "site": "src/components/task-row.tsx" }],
                "score_0_10000": 9500,
                "files": ["src/components/task-row.tsx"],
            }),
            json!({
                "kind": "forbidden_writer",
                "scope": "task_state",
                "severity": "high",
                "summary": "task_state is being written outside the owner",
                "trust_tier": "trusted",
                "leverage_class": "boundary_discipline",
                "likely_fix_sites": [{ "site": "src/store/task-state.ts" }],
                "concept_id": "task_state",
                "score_0_10000": 9200,
                "files": ["src/store/task-state.ts"],
            }),
        ],
        experimental_findings: Vec::new(),
        missing_obligations: Vec::new(),
        watchpoints: Vec::new(),
        resolved_findings: Vec::new(),
        changed_files: vec!["src/components/task-row.tsx".to_string()],
        changed_concepts: Vec::new(),
        decision: Some("fail".to_string()),
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9200 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9200 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: Some(false),
        limit: 1,
    })
    .expect("agent brief");

    assert_eq!(brief["decision"], "block");
    assert_eq!(brief["primary_target_count"], 1);
    assert_eq!(brief["primary_targets"][0]["blocking"], true);
    assert_eq!(brief["primary_targets"][0]["kind"], "forbidden_writer");
}

#[test]
fn patch_brief_keeps_clone_followthrough_targets_fixable() {
    let brief = build_agent_brief(AgentBriefInput {
        mode: AgentBriefMode::Patch,
        repo_shape: json!({
            "primary_archetype": "react_frontend",
            "effective_archetypes": ["react_frontend"],
            "boundary_roots": [],
            "starter_rules_toml": null,
        }),
        findings: vec![json!({
            "kind": "clone_propagation_drift",
            "scope": "src/source.ts|src/copy.ts",
            "severity": "high",
            "summary": "The changed clone path no longer matches its unchanged sibling",
            "trust_tier": "trusted",
            "presentation_class": "structural_debt",
            "leverage_class": "architecture_signal",
            "score_0_10000": 9100,
            "files": ["src/source.ts", "src/copy.ts"],
            "evidence": [
                "changed clone member: src/source.ts::renderStatus",
                "unchanged clone sibling: src/copy.ts::renderStatus"
            ],
        })],
        experimental_findings: Vec::new(),
        missing_obligations: Vec::new(),
        watchpoints: Vec::new(),
        resolved_findings: Vec::new(),
        changed_files: vec!["src/source.ts".to_string()],
        changed_concepts: Vec::new(),
        decision: Some("fail".to_string()),
        summary: None,
        confidence: json!({ "scan_confidence_0_10000": 9300 }),
        scan_trust: json!({ "overall_confidence_0_10000": 9300 }),
        freshness: json!({ "baseline_loaded": true }),
        strict: Some(false),
        limit: 3,
    })
    .expect("agent brief");

    assert_eq!(brief["primary_target_count"], 1);
    assert_eq!(
        brief["primary_targets"][0]["kind"],
        "clone_propagation_drift"
    );
    assert_eq!(
        brief["primary_targets"][0]["repair_packet"]["required_fields"]["repair_surface"],
        true
    );
}
