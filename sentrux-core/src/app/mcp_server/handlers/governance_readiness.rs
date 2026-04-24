use super::load_persisted_baseline;
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) struct GovernanceReadinessItem {
    pub(crate) scope: &'static str,
    pub(crate) file: &'static str,
    pub(crate) check_message: &'static str,
    pub(crate) check_fix_hint: &'static str,
    pub(crate) check_first_cut: &'static str,
    pub(crate) check_evidence: Vec<String>,
    pub(crate) findings_summary: &'static str,
    pub(crate) findings_first_cut: &'static str,
    pub(crate) findings_evidence: Vec<String>,
}

pub(crate) fn governance_readiness_items(root: &Path) -> Vec<GovernanceReadinessItem> {
    let mut items = Vec::new();
    let rules_path = root.join(".sentrux").join("rules.toml");
    if !rules_path.exists() {
        items.push(missing_rules_item());
    }

    let baseline_path = crate::metrics::arch::baseline_path(root);
    if !baseline_path.exists() {
        items.push(missing_baseline_item());
    } else if let Err(error) = load_persisted_baseline(root) {
        items.push(invalid_baseline_item(error));
    }

    items
}

fn missing_rules_item() -> GovernanceReadinessItem {
    GovernanceReadinessItem {
        scope: "missing_sentrux_rules",
        file: ".sentrux/rules.toml",
        check_message: "Sentrux rules are not configured, so architectural checks are running without repo-specific policy.",
        check_fix_hint: "Create .sentrux/rules.toml with starter project exclusions and the first enforced concept or module contract.",
        check_first_cut: "Create .sentrux/rules.toml, start with a [project] section, then add the first concept/module contract before relying on the gate.",
        check_evidence: evidence(&[
            "starter file: .sentrux/rules.toml",
            "CI command after setup: sentrux gate",
        ]),
        findings_summary: "Sentrux rules are not configured, so repo-specific architecture policy is not enforceable yet.",
        findings_first_cut: "Create .sentrux/rules.toml with starter project exclusions and the first enforced concept or module contract.",
        findings_evidence: evidence(&[
            "starter file: .sentrux/rules.toml",
            "example section: [project]",
        ]),
    }
}

fn missing_baseline_item() -> GovernanceReadinessItem {
    GovernanceReadinessItem {
        scope: "missing_sentrux_baseline",
        file: ".sentrux/baseline.json",
        check_message: "Sentrux has no saved baseline, so pre-merge gate comparisons cannot prove whether this patch degraded architecture health.",
        check_fix_hint: "Run `sentrux gate --save` after reviewing the current repository state, then commit .sentrux/baseline.json.",
        check_first_cut: "Run `sentrux gate --save`, review the generated baseline, commit it, and use `sentrux gate` in pre-merge checks.",
        check_evidence: evidence(&[
            "setup command: sentrux gate --save",
            "CI command after baseline commit: sentrux gate",
        ]),
        findings_summary: "Sentrux has no saved baseline, so pre-merge gate comparisons are not enforceable yet.",
        findings_first_cut: "Run `sentrux gate --save`, review the generated baseline, and commit .sentrux/baseline.json.",
        findings_evidence: evidence(&[
            "setup command: sentrux gate --save",
            "CI command after baseline commit: sentrux gate",
        ]),
    }
}

fn invalid_baseline_item(error: String) -> GovernanceReadinessItem {
    GovernanceReadinessItem {
        scope: "invalid_sentrux_baseline",
        file: ".sentrux/baseline.json",
        check_message: "Sentrux could not read the saved baseline, so pre-merge gate comparisons may fail before evaluating patch risk.",
        check_fix_hint: "Regenerate .sentrux/baseline.json with `sentrux gate --save` after confirming the current repository state is acceptable.",
        check_first_cut: "Inspect the baseline parse error, regenerate with `sentrux gate --save`, and commit the repaired baseline.",
        check_evidence: vec![format!("baseline load error: {error}")],
        findings_summary: "Sentrux could not read the saved baseline, so gate comparisons may fail before evaluating patch risk.",
        findings_first_cut: "Regenerate .sentrux/baseline.json with `sentrux gate --save` after confirming the current repository state is acceptable.",
        findings_evidence: vec![format!("baseline load error: {error}")],
    }
}

pub(crate) fn starter_rules_toml() -> &'static str {
    r#"[project]
primary_language = "typescript"
exclude = ["node_modules/**", "dist/**", "build/**", "coverage/**"]

# Add the first enforced concept or module contract after reviewing project shape.
"#
}

fn evidence(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}
