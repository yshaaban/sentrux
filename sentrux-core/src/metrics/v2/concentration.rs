//! Concentration-risk context analyzer for coordination hotspots.

use super::relevant_production_writes;
use crate::analysis::semantic::SemanticSnapshot;
use crate::metrics::evo::EvolutionReport;
use crate::metrics::rules::RulesConfig;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

const SIDE_EFFECT_PATTERNS: &[&str] = &[
    "setStore(",
    "dispatch(",
    ".dispatch(",
    "emit(",
    ".emit(",
    "invoke(",
    ".invoke(",
    "writeFile(",
    "readFile(",
    "spawn(",
    "exec(",
    "postMessage(",
];
const TIMER_RETRY_PATTERNS: &[&str] = &[
    "setTimeout(",
    "setInterval(",
    "clearTimeout(",
    "clearInterval(",
    "retry(",
    ".retry(",
    "backoff(",
    ".backoff(",
    "debounce(",
    ".debounce(",
    "throttle(",
    ".throttle(",
];
const ASYNC_BRANCH_PATTERNS: &[&str] = &[
    "await ",
    ".then(",
    ".catch(",
    ".finally(",
    "Promise.all(",
    "Promise.race(",
    "Promise.any(",
    "switch (",
    "try {",
];

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct ConcentrationReport {
    pub path: String,
    pub score_0_10000: u32,
    pub authority_breadth: u32,
    pub side_effect_breadth: u32,
    pub timer_retry_weight: u32,
    pub async_branch_weight: u32,
    pub max_complexity: u32,
    pub churn_commits: u32,
    pub hotspot_risk: u64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct ConcentrationFinding {
    pub kind: String,
    pub severity: String,
    pub path: String,
    pub score_0_10000: u32,
    pub summary: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ConcentrationHistory {
    pub churn_commits: HashMap<String, u32>,
    pub hotspot_risk: HashMap<String, u64>,
}

impl From<&EvolutionReport> for ConcentrationHistory {
    fn from(report: &EvolutionReport) -> Self {
        let churn_commits = report
            .churn
            .iter()
            .map(|(path, churn)| (path.clone(), churn.commit_count))
            .collect::<HashMap<_, _>>();
        let hotspot_risk = report
            .hotspots
            .iter()
            .map(|hotspot| (hotspot.file.clone(), hotspot.risk_score))
            .collect::<HashMap<_, _>>();

        Self {
            churn_commits,
            hotspot_risk,
        }
    }
}

pub fn build_concentration_reports(
    root: &Path,
    file_paths: &BTreeSet<String>,
    complexity_map: &HashMap<String, u32>,
    config: &RulesConfig,
    semantic: Option<&SemanticSnapshot>,
    history: Option<&ConcentrationHistory>,
) -> Vec<ConcentrationReport> {
    let authority_by_file = authority_breadth_by_file(config, semantic);
    let mut reports = file_paths
        .iter()
        .map(|path| {
            let contents = std::fs::read_to_string(root.join(path)).unwrap_or_default();
            let signal_source = scrub_non_code_regions(&contents);
            let side_effect_api_weight =
                capped_pattern_hits(&signal_source, SIDE_EFFECT_PATTERNS, 3);
            let timer_retry_weight = capped_pattern_hits(&signal_source, TIMER_RETRY_PATTERNS, 4);
            let async_branch_weight = capped_pattern_hits(&signal_source, ASYNC_BRANCH_PATTERNS, 6);
            let authority_breadth = authority_by_file.get(path).copied().unwrap_or(0);
            let side_effect_breadth = side_effect_api_weight
                + semantic
                    .map(|semantic| {
                        semantic
                            .writes
                            .iter()
                            .filter(|write| write.path == *path)
                            .map(|write| write.symbol_name.clone())
                            .collect::<HashSet<_>>()
                            .len() as u32
                    })
                    .unwrap_or(0);
            let max_complexity = complexity_map.get(path).copied().unwrap_or(0);
            let churn_commits = history
                .and_then(|history| history.churn_commits.get(path).copied())
                .unwrap_or(0);
            let hotspot_risk = history
                .and_then(|history| history.hotspot_risk.get(path).copied())
                .unwrap_or(0);
            let history_available = history
                .map(|history| {
                    history.churn_commits.contains_key(path)
                        || history.hotspot_risk.contains_key(path)
                })
                .unwrap_or(false);

            let score_0_10000 = concentration_score_0_10000(
                authority_breadth,
                side_effect_breadth,
                timer_retry_weight,
                async_branch_weight,
                max_complexity,
                churn_commits,
                hotspot_risk,
                history_available,
            );

            ConcentrationReport {
                path: path.clone(),
                score_0_10000,
                authority_breadth,
                side_effect_breadth,
                timer_retry_weight,
                async_branch_weight,
                max_complexity,
                churn_commits,
                hotspot_risk,
                reasons: concentration_reasons(
                    authority_breadth,
                    side_effect_breadth,
                    timer_retry_weight,
                    async_branch_weight,
                    max_complexity,
                    churn_commits,
                    hotspot_risk,
                ),
            }
        })
        .filter(|report| report.score_0_10000 > 0)
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| {
        right
            .score_0_10000
            .cmp(&left.score_0_10000)
            .then_with(|| right.hotspot_risk.cmp(&left.hotspot_risk))
            .then_with(|| right.max_complexity.cmp(&left.max_complexity))
            .then_with(|| left.path.cmp(&right.path))
    });
    reports
}

pub fn build_concentration_findings(
    reports: &[ConcentrationReport],
    limit: usize,
) -> Vec<ConcentrationFinding> {
    reports
        .iter()
        .filter(|report| report.score_0_10000 >= 3000)
        .take(limit)
        .map(|report| {
            let severity = if report.score_0_10000 >= 6500 {
                "high"
            } else {
                "medium"
            };
            ConcentrationFinding {
                kind: "coordination_hotspot".to_string(),
                severity: severity.to_string(),
                path: report.path.clone(),
                score_0_10000: report.score_0_10000,
                summary: format!(
                    "File '{}' is a concentrated coordination hotspot",
                    report.path
                ),
                reasons: report.reasons.clone(),
            }
        })
        .collect()
}

fn authority_breadth_by_file(
    config: &RulesConfig,
    semantic: Option<&SemanticSnapshot>,
) -> HashMap<String, u32> {
    let Some(semantic) = semantic else {
        return HashMap::new();
    };

    let mut concepts_by_file: HashMap<String, HashSet<String>> = HashMap::new();
    for concept in &config.concept {
        for write in relevant_production_writes(concept, semantic) {
            concepts_by_file
                .entry(write.path.clone())
                .or_default()
                .insert(concept.id.clone());
        }
    }

    concepts_by_file
        .into_iter()
        .map(|(path, concepts)| (path, concepts.len() as u32))
        .collect()
}

fn capped_pattern_hits(contents: &str, patterns: &[&str], cap_per_pattern: usize) -> u32 {
    patterns
        .iter()
        .map(|pattern| contents.matches(pattern).take(cap_per_pattern).count() as u32)
        .sum()
}

fn scrub_non_code_regions(contents: &str) -> String {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum State {
        Code,
        LineComment,
        BlockComment,
        SingleQuote,
        DoubleQuote,
        Template,
    }

    let mut scrubbed = String::with_capacity(contents.len());
    let mut chars = contents.chars().peekable();
    let mut state = State::Code;
    let mut escaping = false;

    while let Some(character) = chars.next() {
        match state {
            State::Code => {
                let next = chars.peek().copied();
                match (character, next) {
                    ('/', Some('/')) => {
                        scrubbed.push(' ');
                        scrubbed.push(' ');
                        chars.next();
                        state = State::LineComment;
                    }
                    ('/', Some('*')) => {
                        scrubbed.push(' ');
                        scrubbed.push(' ');
                        chars.next();
                        state = State::BlockComment;
                    }
                    ('\'', _) => {
                        scrubbed.push(' ');
                        state = State::SingleQuote;
                        escaping = false;
                    }
                    ('"', _) => {
                        scrubbed.push(' ');
                        state = State::DoubleQuote;
                        escaping = false;
                    }
                    ('`', _) => {
                        scrubbed.push(' ');
                        state = State::Template;
                        escaping = false;
                    }
                    _ => scrubbed.push(character),
                }
            }
            State::LineComment => {
                if character == '\n' {
                    scrubbed.push('\n');
                    state = State::Code;
                } else {
                    scrubbed.push(' ');
                }
            }
            State::BlockComment => {
                let next = chars.peek().copied();
                if character == '*' && next == Some('/') {
                    scrubbed.push(' ');
                    scrubbed.push(' ');
                    chars.next();
                    state = State::Code;
                } else if character == '\n' {
                    scrubbed.push('\n');
                } else {
                    scrubbed.push(' ');
                }
            }
            State::SingleQuote => match character {
                '\n' => {
                    scrubbed.push('\n');
                    state = State::Code;
                    escaping = false;
                }
                '\\' if !escaping => {
                    scrubbed.push(' ');
                    escaping = true;
                }
                '\'' if !escaping => {
                    scrubbed.push(' ');
                    state = State::Code;
                }
                _ => {
                    scrubbed.push(' ');
                    escaping = false;
                }
            },
            State::DoubleQuote => match character {
                '\n' => {
                    scrubbed.push('\n');
                    state = State::Code;
                    escaping = false;
                }
                '\\' if !escaping => {
                    scrubbed.push(' ');
                    escaping = true;
                }
                '"' if !escaping => {
                    scrubbed.push(' ');
                    state = State::Code;
                }
                _ => {
                    scrubbed.push(' ');
                    escaping = false;
                }
            },
            State::Template => match character {
                '\n' => {
                    scrubbed.push('\n');
                    escaping = false;
                }
                '\\' if !escaping => {
                    scrubbed.push(' ');
                    escaping = true;
                }
                '`' if !escaping => {
                    scrubbed.push(' ');
                    state = State::Code;
                }
                _ => {
                    scrubbed.push(' ');
                    escaping = false;
                }
            },
        }
    }

    scrubbed
}

fn concentration_score_0_10000(
    authority_breadth: u32,
    side_effect_breadth: u32,
    timer_retry_weight: u32,
    async_branch_weight: u32,
    max_complexity: u32,
    churn_commits: u32,
    hotspot_risk: u64,
    history_available: bool,
) -> u32 {
    let authority = normalize(authority_breadth, 3.0);
    let side_effects = normalize(side_effect_breadth, 12.0);
    let timer_retry = normalize(timer_retry_weight, 8.0);
    let async_branch = normalize(async_branch_weight, 12.0);
    let complexity = normalize(max_complexity, 25.0);
    let churn = normalize(churn_commits, 20.0).max(normalize_u64(hotspot_risk, 250.0));
    let static_weighted = (authority * 0.22)
        + (side_effects * 0.20)
        + (timer_retry * 0.16)
        + (async_branch * 0.14)
        + (complexity * 0.14);
    let history_weighted = if history_available { churn * 0.14 } else { 0.0 };
    let active_weight = if history_available { 1.0 } else { 0.86 };
    if active_weight <= f64::EPSILON {
        return 0;
    }

    (((static_weighted + history_weighted) / active_weight) * 10000.0).round() as u32
}

fn normalize(value: u32, max_value: f64) -> f64 {
    ((value as f64) / max_value).clamp(0.0, 1.0)
}

fn normalize_u64(value: u64, max_value: f64) -> f64 {
    ((value as f64) / max_value).clamp(0.0, 1.0)
}

fn concentration_reasons(
    authority_breadth: u32,
    side_effect_breadth: u32,
    timer_retry_weight: u32,
    async_branch_weight: u32,
    max_complexity: u32,
    churn_commits: u32,
    hotspot_risk: u64,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if authority_breadth > 0 {
        reasons.push(format!(
            "writes {} configured concept(s)",
            authority_breadth
        ));
    }
    if side_effect_breadth > 0 {
        reasons.push(format!(
            "touches {} side-effect target(s)",
            side_effect_breadth
        ));
    }
    if timer_retry_weight > 0 {
        reasons.push(format!(
            "contains {} timer/retry coordination signal(s)",
            timer_retry_weight
        ));
    }
    if async_branch_weight > 0 {
        reasons.push(format!(
            "contains {} async/branching control signal(s)",
            async_branch_weight
        ));
    }
    if max_complexity >= 10 {
        reasons.push(format!("max cyclomatic complexity {}", max_complexity));
    }
    if churn_commits > 0 {
        reasons.push(format!("changed in {} recent commit(s)", churn_commits));
    }
    if hotspot_risk > 0 {
        reasons.push(format!("git hotspot risk {}", hotspot_risk));
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::{
        build_concentration_findings, build_concentration_reports, capped_pattern_hits,
        scrub_non_code_regions, ConcentrationHistory, TIMER_RETRY_PATTERNS,
    };
    use crate::analysis::semantic::{
        ProjectModel, SemanticCapability, SemanticSnapshot, WriteFact,
    };
    use crate::metrics::rules::RulesConfig;
    use std::collections::{BTreeSet, HashMap};
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sentrux-concentration-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(root: &Path, relative_path: &str, contents: &str) {
        let absolute_path = root.join(relative_path);
        if let Some(parent) = absolute_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directories");
        }
        std::fs::write(&absolute_path, contents).expect("write file");
    }

    #[test]
    fn concentration_reports_rank_coordination_file_above_simple_file() {
        let root = temp_root("ranking");
        write_file(
            &root,
            "src/lease.ts",
            "export async function lease(): Promise<void> {\n  setTimeout(() => {}, 10);\n  await Promise.resolve();\n  if (true) {\n    retry();\n  }\n  dispatch('x');\n}\n",
        );
        write_file(
            &root,
            "src/simple.ts",
            "export function simple(): number { return 1; }\n",
        );
        let file_paths = BTreeSet::from(["src/lease.ts".to_string(), "src/simple.ts".to_string()]);
        let complexity = HashMap::from([
            ("src/lease.ts".to_string(), 18),
            ("src/simple.ts".to_string(), 1),
        ]);
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "lease_control"
                anchors = ["src/lease.ts::store.leaseState"]
                allowed_writers = ["src/lease.ts::*"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 2,
            capabilities: vec![SemanticCapability::Writes],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: vec![WriteFact {
                path: "src/lease.ts".to_string(),
                symbol_name: "store.leaseState".to_string(),
                write_kind: "store_call".to_string(),
                line: 2,
            }],
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };
        let history = ConcentrationHistory {
            churn_commits: HashMap::from([
                ("src/lease.ts".to_string(), 9),
                ("src/simple.ts".to_string(), 1),
            ]),
            hotspot_risk: HashMap::from([
                ("src/lease.ts".to_string(), 120),
                ("src/simple.ts".to_string(), 0),
            ]),
        };

        let reports = build_concentration_reports(
            &root,
            &file_paths,
            &complexity,
            &config,
            Some(&semantic),
            Some(&history),
        );

        assert_eq!(reports[0].path, "src/lease.ts");
        assert!(reports[0].score_0_10000 > reports[1].score_0_10000);
        assert!(reports[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("timer/retry")));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn concentration_findings_only_emit_for_hot_files() {
        let reports = vec![
            super::ConcentrationReport {
                path: "src/lease.ts".to_string(),
                score_0_10000: 8200,
                reasons: vec!["writes 1 configured concept(s)".to_string()],
                ..Default::default()
            },
            super::ConcentrationReport {
                path: "src/simple.ts".to_string(),
                score_0_10000: 2100,
                reasons: vec!["max cyclomatic complexity 1".to_string()],
                ..Default::default()
            },
        ];

        let findings = build_concentration_findings(&reports, 10);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, "coordination_hotspot");
        assert_eq!(findings[0].severity, "high");
        assert_eq!(findings[0].path, "src/lease.ts");
    }

    #[test]
    fn concentration_ignores_comment_and_string_noise() {
        let scrubbed = scrub_non_code_regions(
            r#"
                // dispatch('commented');
                const sample = "setTimeout(() => {})";
                /*
                  retry();
                  Promise.all([]);
                */
                export function real(): void {
                    dispatch("live");
                    awaitLater();
                }
            "#,
        );

        assert!(!scrubbed.contains("commented"));
        assert!(!scrubbed.contains("setTimeout"));
        assert!(!scrubbed.contains("retry()"));
        assert!(scrubbed.contains("dispatch("));
        assert!(scrubbed.contains("awaitLater("));
    }

    #[test]
    fn concentration_timer_patterns_ignore_identifier_substrings() {
        let scrubbed = scrub_non_code_regions(
            r#"
                export function config(): void {
                    const retryCount = 1;
                    const throttleMs = 10;
                    const backoffDelay = 25;
                }
            "#,
        );

        assert_eq!(capped_pattern_hits(&scrubbed, TIMER_RETRY_PATTERNS, 10), 0);
    }

    #[test]
    fn concentration_findings_do_not_require_git_history() {
        let root = temp_root("no-history-finding");
        write_file(
            &root,
            "src/lease.ts",
            "export async function lease(): Promise<void> {\n  setTimeout(() => {}, 10);\n  await refresh();\n  retry();\n  dispatch('x');\n}\n",
        );
        let file_paths = BTreeSet::from(["src/lease.ts".to_string()]);
        let complexity = HashMap::from([("src/lease.ts".to_string(), 18)]);
        let config: RulesConfig = toml::from_str(
            r#"
                [[concept]]
                id = "lease_control"
                anchors = ["src/lease.ts::store.leaseState"]
                allowed_writers = ["src/lease.ts::*"]
            "#,
        )
        .expect("rules config");
        let semantic = SemanticSnapshot {
            project: ProjectModel::default(),
            analyzed_files: 1,
            capabilities: vec![SemanticCapability::Writes],
            files: Vec::new(),
            symbols: Vec::new(),
            reads: Vec::new(),
            writes: vec![WriteFact {
                path: "src/lease.ts".to_string(),
                symbol_name: "store.leaseState".to_string(),
                write_kind: "store_call".to_string(),
                line: 2,
            }],
            closed_domains: Vec::new(),
            closed_domain_sites: Vec::new(),
        };

        let reports = build_concentration_reports(
            &root,
            &file_paths,
            &complexity,
            &config,
            Some(&semantic),
            None,
        );
        let findings = build_concentration_findings(&reports, 10);

        assert_eq!(reports.len(), 1);
        assert!(reports[0].score_0_10000 >= 3000);
        assert_eq!(findings.len(), 1);

        let _ = std::fs::remove_dir_all(root);
    }
}
