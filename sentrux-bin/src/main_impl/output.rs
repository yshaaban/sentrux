use serde_json::Value;

pub(crate) fn print_v2_gate_save(payload: &Value) {
    println!("sentrux gate — v2 baseline saved\n");
    if let Some(path) = payload
        .get("session_v2_baseline_path")
        .and_then(|value| value.as_str())
    {
        println!("V2 session baseline: {path}");
    }
    if let Some(count) = payload
        .get("session_finding_count")
        .and_then(|value| value.as_u64())
    {
        println!("Tracked findings: {count}");
    }
    if let Some(count) = payload
        .get("suppressed_finding_count")
        .and_then(|value| value.as_u64())
    {
        println!("Suppressed findings: {count}");
    }
    if let Some(count) = payload
        .get("expired_suppression_match_count")
        .and_then(|value| value.as_u64())
    {
        println!("Expired suppression matches: {count}");
    }
    print_cli_confidence_summary(payload);
    if let Some(path) = payload
        .get("baseline_path")
        .and_then(|value| value.as_str())
    {
        println!("Legacy structural baseline: {path}");
    }
    if let Some(quality_signal) = payload
        .get("quality_signal")
        .and_then(|value| value.as_u64())
    {
        println!("Supporting structural context: {quality_signal}");
    }
    if let Some(error) = diagnostics_error(payload, "semantic") {
        println!("\nSemantic note: {error}");
    }
    if let Some(message) = payload.get("message").and_then(|value| value.as_str()) {
        println!("\n{message}");
    }
}

pub(crate) fn print_v2_gate_result(payload: &Value) -> i32 {
    let decision = payload
        .get("decision")
        .and_then(|value| value.as_str())
        .unwrap_or("fail");
    let summary = payload
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("Touched-concept gate finished.");
    let changed_files = payload
        .get("changed_files")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let blocking_findings = payload
        .get("blocking_findings")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let missing_obligations = payload
        .get("missing_obligations")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let introduced_findings = payload
        .get("introduced_findings")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let introduced_nonblocking_findings = introduced_findings
        .iter()
        .filter(|finding| severity_of_value(finding) != "high")
        .cloned()
        .collect::<Vec<_>>();
    let suppression_hits = payload
        .get("suppression_hits")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let expired_suppressions = payload
        .get("expired_suppressions")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let changed_concepts = string_array_from_value(payload.get("changed_concepts"));

    println!("sentrux gate — touched-concept regression check\n");
    print_v2_gate_summary(
        payload,
        decision,
        summary,
        &changed_files,
        &changed_concepts,
        &introduced_findings,
        &blocking_findings,
        &missing_obligations,
    );
    print_v2_gate_suppression_sections(&suppression_hits, &expired_suppressions);
    print_v2_gate_findings(&blocking_findings, &introduced_nonblocking_findings);
    print_v2_gate_obligations(&missing_obligations);
    print_v2_gate_notes(payload);
    exit_code_for_gate_decision(decision)
}

fn print_v2_gate_summary(
    payload: &Value,
    decision: &str,
    summary: &str,
    changed_files: &[Value],
    changed_concepts: &[String],
    introduced_findings: &[Value],
    blocking_findings: &[Value],
    missing_obligations: &[Value],
) {
    println!("Decision:     {decision}");
    println!("Summary:      {summary}");
    println!("Changed files: {}", changed_files.len());
    if !changed_concepts.is_empty() {
        print_string_section("Changed concepts", changed_concepts, 10);
    }
    print_legacy_baseline_delta(payload);
    println!("Introduced findings: {}", introduced_findings.len());
    println!("Blocking findings:  {}", blocking_findings.len());
    println!("Missing obligations: {}", missing_obligations.len());
    if let Some(score) = payload
        .get("obligation_completeness_0_10000")
        .and_then(|value| value.as_u64())
    {
        println!("Obligation completeness: {score}/10000");
    }
    print_scan_trust_summary(payload);
    print_cli_confidence_summary(payload);
}

fn print_v2_gate_suppression_sections(suppression_hits: &[Value], expired_suppressions: &[Value]) {
    println!("Suppression hits: {}", suppression_hits.len());
    println!("Expired suppressions: {}", expired_suppressions.len());
    print_gate_suppression_section("Suppression hits", suppression_hits);
    print_gate_suppression_section("Expired suppressions", expired_suppressions);
}

fn print_gate_suppression_section(title: &str, matches: &[Value]) {
    if matches.is_empty() {
        return;
    }

    println!("\n{title}:");
    for matched in matches.iter().take(10) {
        print_cli_suppression_match(matched);
    }
}

fn print_v2_gate_findings(blocking_findings: &[Value], introduced_nonblocking_findings: &[Value]) {
    print_gate_finding_section("Blocking findings", blocking_findings);
    print_gate_finding_section(
        "Introduced findings (non-blocking)",
        introduced_nonblocking_findings,
    );
}

fn print_gate_finding_section(title: &str, findings: &[Value]) {
    if findings.is_empty() {
        return;
    }

    println!("\n{title}:");
    for finding in findings.iter().take(10) {
        print_cli_finding(finding);
    }
}

fn print_v2_gate_obligations(missing_obligations: &[Value]) {
    if missing_obligations.is_empty() {
        return;
    }

    println!("\nMissing obligations:");
    for obligation in missing_obligations.iter().take(10) {
        print_cli_obligation(obligation);
    }
}

fn print_v2_gate_notes(payload: &Value) {
    if let Some(error) = diagnostics_error(payload, "semantic") {
        println!("\nSemantic note: {error}");
    }
    if let Some(error) = diagnostics_error(payload, "baseline") {
        println!("Baseline note: {error}");
    }
}

fn exit_code_for_gate_decision(decision: &str) -> i32 {
    if decision == "pass" {
        0
    } else {
        1
    }
}

fn print_cli_finding(finding: &Value) {
    let severity = finding
        .get("severity")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let summary = finding
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("finding");
    println!("  - [{severity}] {summary}");
}

fn print_cli_obligation(obligation: &Value) {
    let summary = obligation
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("missing obligation");
    let missing_count = obligation
        .get("missing_sites")
        .and_then(|value| value.as_array())
        .map(|sites| sites.len())
        .unwrap_or(0);
    println!("  - {summary} ({missing_count} missing site(s))");
}

fn print_cli_suppression_match(matched: &Value) {
    let kind = matched
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or("*");
    let concept = matched
        .get("concept")
        .and_then(|value| value.as_str())
        .unwrap_or("-");
    let file = matched
        .get("file")
        .and_then(|value| value.as_str())
        .unwrap_or("-");
    let count = matched
        .get("matched_finding_count")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let reason = matched
        .get("reason")
        .and_then(|value| value.as_str())
        .unwrap_or("suppressed");
    println!("  - kind={kind} concept={concept} file={file} count={count} reason={reason}");
}

fn diagnostics_error<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload
        .get("diagnostics")
        .and_then(|value| value.get("errors"))
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
}

fn severity_of_value(value: &Value) -> &str {
    value
        .get("severity")
        .and_then(|severity| severity.as_str())
        .unwrap_or("low")
}

fn print_legacy_baseline_delta(payload: &Value) {
    let Some(baseline_delta) = payload
        .get("baseline_delta")
        .and_then(|value| value.as_object())
    else {
        return;
    };
    if !baseline_delta
        .get("available")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return;
    }

    let signal_before = baseline_delta
        .get("signal_before")
        .and_then(|value| value.as_i64());
    let signal_after = baseline_delta
        .get("signal_after")
        .and_then(|value| value.as_i64());
    let signal_delta = baseline_delta
        .get("signal_delta")
        .and_then(|value| value.as_i64());
    if let (Some(before), Some(after), Some(delta)) = (signal_before, signal_after, signal_delta) {
        println!("Supporting structural delta: {before} -> {after} ({delta:+})");
    }

    let coupling_before = baseline_delta
        .get("coupling_before")
        .and_then(|value| value.as_f64());
    let coupling_after = baseline_delta
        .get("coupling_after")
        .and_then(|value| value.as_f64());
    if let (Some(before), Some(after)) = (coupling_before, coupling_after) {
        println!("Coupling:     {:.2} -> {:.2}", before, after);
    }

    let cycles_before = baseline_delta
        .get("cycles_before")
        .and_then(|value| value.as_i64());
    let cycles_after = baseline_delta
        .get("cycles_after")
        .and_then(|value| value.as_i64());
    if let (Some(before), Some(after)) = (cycles_before, cycles_after) {
        println!("Cycles:       {before} -> {after}");
    }
}

fn print_scan_trust_summary(payload: &Value) {
    let Some(scan_trust) = payload
        .get("scan_trust")
        .and_then(|value| value.as_object())
    else {
        return;
    };

    let overall_confidence = scan_trust
        .get("overall_confidence_0_10000")
        .and_then(|value| value.as_u64());
    let scope_coverage = scan_trust
        .get("scope_coverage_0_10000")
        .and_then(|value| value.as_u64());
    let resolution = scan_trust
        .get("resolution")
        .and_then(|value| value.as_object());
    let resolved = resolution
        .and_then(|value| value.get("resolved"))
        .and_then(|value| value.as_u64());
    let unresolved_internal = resolution
        .and_then(|value| value.get("unresolved_internal"))
        .and_then(|value| value.as_u64());
    let unresolved_external = resolution
        .and_then(|value| value.get("unresolved_external"))
        .and_then(|value| value.as_u64());

    if let Some(overall_confidence) = overall_confidence {
        println!("Scan confidence: {overall_confidence}/10000");
    }
    if let Some(scope_coverage) = scope_coverage {
        println!("Scope coverage:  {scope_coverage}/10000");
    }
    if resolved.is_some() || unresolved_internal.is_some() || unresolved_external.is_some() {
        println!(
            "Resolution:      resolved {}, unresolved internal {}, unresolved external {}",
            resolved.unwrap_or(0),
            unresolved_internal.unwrap_or(0),
            unresolved_external.unwrap_or(0),
        );
    }

    if let Some(partial) = scan_trust.get("partial").and_then(|value| value.as_bool()) {
        if partial {
            println!("Scan note: partial coverage");
        }
    }
    if let Some(truncated) = scan_trust
        .get("truncated")
        .and_then(|value| value.as_bool())
    {
        if truncated {
            println!("Scan note: truncated results");
        }
    }
    if let Some(fallback_reason) = scan_trust
        .get("fallback_reason")
        .and_then(|value| value.as_str())
    {
        println!("Scan note: {fallback_reason}");
    }
}

fn print_cli_confidence_summary(payload: &Value) {
    let Some(confidence) = payload
        .get("confidence")
        .and_then(|value| value.as_object())
    else {
        return;
    };

    if let Some(rule_coverage) = confidence
        .get("rule_coverage_0_10000")
        .and_then(|value| value.as_u64())
    {
        println!("Rule coverage:  {rule_coverage}/10000");
    }

    let session_baseline = confidence
        .get("session_baseline")
        .and_then(|value| value.as_object());
    if let Some(status) = session_baseline {
        let loaded = status
            .get("loaded")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let compatible = status
            .get("compatible")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let schema_version = status
            .get("schema_version")
            .and_then(|value| value.as_u64());
        let error = status.get("error").and_then(|value| value.as_str());

        if loaded {
            if let Some(version) = schema_version {
                let compatibility = if compatible {
                    "compatible"
                } else {
                    "incompatible"
                };
                println!("Session baseline: v{version} ({compatibility})");
            } else {
                println!("Session baseline: loaded");
            }
        } else {
            println!("Session baseline: unavailable");
        }

        if let Some(error) = error {
            println!("Session baseline note: {error}");
        }
    }
}

fn print_string_section(title: &str, items: &[String], limit: usize) {
    println!("\n{title}:");
    for item in items.iter().take(limit) {
        println!("  - {item}");
    }
    if items.len() > limit {
        println!("  - ... ({} more)", items.len() - limit);
    }
}

fn string_array_from_value(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|item| item.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}
