//! Concentration-risk context analyzer for coordination hotspots.

use super::{relevant_production_writes, FindingSeverity};
use crate::analysis::semantic::SemanticSnapshot;
use crate::metrics::evo::EvolutionReport;
use crate::metrics::rules::RulesConfig;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

const AUTHORITY_NORMALIZATION_MAX: f64 = 3.0;
const SIDE_EFFECT_NORMALIZATION_MAX: f64 = 12.0;
const TIMER_RETRY_NORMALIZATION_MAX: f64 = 8.0;
const ASYNC_BRANCH_NORMALIZATION_MAX: f64 = 12.0;
const COMPLEXITY_NORMALIZATION_MAX: f64 = 25.0;
const CHURN_NORMALIZATION_MAX: f64 = 20.0;
const HOTSPOT_RISK_NORMALIZATION_MAX: f64 = 250.0;

const AUTHORITY_WEIGHT: f64 = 0.22;
const SIDE_EFFECT_WEIGHT: f64 = 0.20;
const TIMER_RETRY_WEIGHT: f64 = 0.16;
const ASYNC_BRANCH_WEIGHT: f64 = 0.14;
const COMPLEXITY_WEIGHT: f64 = 0.14;
const HISTORY_WEIGHT: f64 = 0.14;
const STATIC_ONLY_ACTIVE_WEIGHT: f64 = 0.86;

const HOTSPOT_FINDING_MIN_SCORE: u32 = 3000;
const HOTSPOT_HIGH_SEVERITY_MIN_SCORE: u32 = 6500;

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
    pub severity: FindingSeverity,
    pub path: String,
    pub score_0_10000: u32,
    pub summary: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ConcentrationBuildResult {
    pub reports: Vec<ConcentrationReport>,
    pub read_warnings: Vec<String>,
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
) -> ConcentrationBuildResult {
    let authority_by_file = authority_breadth_by_file(config, semantic);
    let mut read_warnings = Vec::new();
    let mut reports = file_paths
        .iter()
        .map(|path| {
            build_concentration_report(
                root,
                path,
                complexity_map,
                semantic,
                history,
                &authority_by_file,
                &mut read_warnings,
            )
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
    ConcentrationBuildResult {
        reports,
        read_warnings,
    }
}

fn build_concentration_report(
    root: &Path,
    path: &str,
    complexity_map: &HashMap<String, u32>,
    semantic: Option<&SemanticSnapshot>,
    history: Option<&ConcentrationHistory>,
    authority_by_file: &HashMap<String, u32>,
    read_warnings: &mut Vec<String>,
) -> ConcentrationReport {
    let contents = match std::fs::read_to_string(root.join(path)) {
        Ok(contents) => contents,
        Err(error) => {
            read_warnings.push(format!(
                "Failed to read concentration source '{}': {error}",
                path
            ));
            String::new()
        }
    };
    let signal_source = scrub_non_code_regions(&contents);
    let side_effect_api_weight = capped_pattern_hits(&signal_source, SIDE_EFFECT_PATTERNS, 3);
    let timer_retry_weight = capped_pattern_hits(&signal_source, TIMER_RETRY_PATTERNS, 4);
    let async_branch_weight = capped_pattern_hits(&signal_source, ASYNC_BRANCH_PATTERNS, 6);
    let authority_breadth = authority_by_file.get(path).copied().unwrap_or(0);
    let side_effect_breadth = side_effect_api_weight + semantic_side_effect_breadth(semantic, path);
    let max_complexity = complexity_map.get(path).copied().unwrap_or(0);
    let churn_commits = history
        .and_then(|history| history.churn_commits.get(path).copied())
        .unwrap_or(0);
    let hotspot_risk = history
        .and_then(|history| history.hotspot_risk.get(path).copied())
        .unwrap_or(0);
    let history_available = history
        .map(|history| {
            history.churn_commits.contains_key(path) || history.hotspot_risk.contains_key(path)
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
        path: path.to_string(),
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
}

fn semantic_side_effect_breadth(semantic: Option<&SemanticSnapshot>, path: &str) -> u32 {
    semantic
        .map(|semantic| {
            semantic
                .writes
                .iter()
                .filter(|write| write.path == path)
                .map(|write| write.symbol_name.clone())
                .collect::<HashSet<_>>()
                .len() as u32
        })
        .unwrap_or(0)
}

pub fn build_concentration_findings(
    reports: &[ConcentrationReport],
    limit: usize,
) -> Vec<ConcentrationFinding> {
    reports
        .iter()
        .filter(|report| report.score_0_10000 >= HOTSPOT_FINDING_MIN_SCORE)
        .take(limit)
        .map(|report| {
            let severity = if report.score_0_10000 >= HOTSPOT_HIGH_SEVERITY_MIN_SCORE {
                FindingSeverity::High
            } else {
                FindingSeverity::Medium
            };
            ConcentrationFinding {
                kind: "coordination_hotspot".to_string(),
                severity,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScrubState {
    Code,
    LineComment,
    BlockComment,
    SingleQuote,
    DoubleQuote,
    Template,
}

fn push_scrubbed_character(scrubbed: &mut String, character: char) {
    if character == '\n' {
        scrubbed.push('\n');
    } else {
        scrubbed.push(' ');
    }
}

fn enter_comment_state(
    scrubbed: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    state: &mut ScrubState,
    next_state: ScrubState,
) {
    scrubbed.push(' ');
    scrubbed.push(' ');
    chars.next();
    *state = next_state;
}

fn handle_code_scrub_state(
    character: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    scrubbed: &mut String,
    state: &mut ScrubState,
    escaping: &mut bool,
) {
    let next = chars.peek().copied();
    match (character, next) {
        ('/', Some('/')) => enter_comment_state(scrubbed, chars, state, ScrubState::LineComment),
        ('/', Some('*')) => enter_comment_state(scrubbed, chars, state, ScrubState::BlockComment),
        ('\'', _) => {
            scrubbed.push(' ');
            *state = ScrubState::SingleQuote;
            *escaping = false;
        }
        ('"', _) => {
            scrubbed.push(' ');
            *state = ScrubState::DoubleQuote;
            *escaping = false;
        }
        ('`', _) => {
            scrubbed.push(' ');
            *state = ScrubState::Template;
            *escaping = false;
        }
        _ => scrubbed.push(character),
    }
}

fn handle_quoted_scrub_state(
    character: char,
    closing_delimiter: char,
    scrubbed: &mut String,
    state: &mut ScrubState,
    escaping: &mut bool,
    preserve_newline: bool,
) {
    match character {
        '\n' => {
            scrubbed.push('\n');
            if !preserve_newline {
                *state = ScrubState::Code;
            }
            *escaping = false;
        }
        '\\' if !*escaping => {
            scrubbed.push(' ');
            *escaping = true;
        }
        current if current == closing_delimiter && !*escaping => {
            scrubbed.push(' ');
            *state = ScrubState::Code;
        }
        _ => {
            scrubbed.push(' ');
            *escaping = false;
        }
    }
}

fn scrub_non_code_regions(contents: &str) -> String {
    let mut scrubbed = String::with_capacity(contents.len());
    let mut chars = contents.chars().peekable();
    let mut state = ScrubState::Code;
    let mut escaping = false;

    while let Some(character) = chars.next() {
        match state {
            ScrubState::Code => handle_code_scrub_state(
                character,
                &mut chars,
                &mut scrubbed,
                &mut state,
                &mut escaping,
            ),
            ScrubState::LineComment => {
                push_scrubbed_character(&mut scrubbed, character);
                if character == '\n' {
                    state = ScrubState::Code;
                }
            }
            ScrubState::BlockComment => {
                let next = chars.peek().copied();
                if character == '*' && next == Some('/') {
                    scrubbed.push(' ');
                    scrubbed.push(' ');
                    chars.next();
                    state = ScrubState::Code;
                } else {
                    push_scrubbed_character(&mut scrubbed, character);
                }
            }
            ScrubState::SingleQuote => handle_quoted_scrub_state(
                character,
                '\'',
                &mut scrubbed,
                &mut state,
                &mut escaping,
                false,
            ),
            ScrubState::DoubleQuote => handle_quoted_scrub_state(
                character,
                '"',
                &mut scrubbed,
                &mut state,
                &mut escaping,
                false,
            ),
            ScrubState::Template => handle_quoted_scrub_state(
                character,
                '`',
                &mut scrubbed,
                &mut state,
                &mut escaping,
                true,
            ),
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
    let authority = normalize(authority_breadth, AUTHORITY_NORMALIZATION_MAX);
    let side_effects = normalize(side_effect_breadth, SIDE_EFFECT_NORMALIZATION_MAX);
    let timer_retry = normalize(timer_retry_weight, TIMER_RETRY_NORMALIZATION_MAX);
    let async_branch = normalize(async_branch_weight, ASYNC_BRANCH_NORMALIZATION_MAX);
    let complexity = normalize(max_complexity, COMPLEXITY_NORMALIZATION_MAX);
    let churn = normalize(churn_commits, CHURN_NORMALIZATION_MAX)
        .max(normalize_u64(hotspot_risk, HOTSPOT_RISK_NORMALIZATION_MAX));
    let static_weighted = (authority * AUTHORITY_WEIGHT)
        + (side_effects * SIDE_EFFECT_WEIGHT)
        + (timer_retry * TIMER_RETRY_WEIGHT)
        + (async_branch * ASYNC_BRANCH_WEIGHT)
        + (complexity * COMPLEXITY_WEIGHT);
    let history_weighted = if history_available {
        churn * HISTORY_WEIGHT
    } else {
        0.0
    };
    let active_weight = if history_available {
        1.0
    } else {
        STATIC_ONLY_ACTIVE_WEIGHT
    };
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
        scrub_non_code_regions, ConcentrationHistory, FindingSeverity, TIMER_RETRY_PATTERNS,
    };
    use crate::analysis::semantic::{
        ProjectModel, SemanticCapability, SemanticSnapshot, WriteFact,
    };
    use crate::metrics::rules::RulesConfig;
    use crate::test_support::temp_root;
    use std::collections::{BTreeSet, HashMap};
    use std::path::Path;

    fn write_file(root: &Path, relative_path: &str, contents: &str) {
        let absolute_path = root.join(relative_path);
        if let Some(parent) = absolute_path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directories");
        }
        std::fs::write(&absolute_path, contents).expect("write file");
    }

    #[test]
    fn concentration_reports_rank_coordination_file_above_simple_file() {
        let root = temp_root("sentrux-concentration", "ranking", &[]);
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
            transition_sites: Vec::new(),
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

        let build_result = build_concentration_reports(
            &root,
            &file_paths,
            &complexity,
            &config,
            Some(&semantic),
            Some(&history),
        );
        let reports = build_result.reports;

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
        assert_eq!(findings[0].severity, FindingSeverity::High);
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
        let root = temp_root("sentrux-concentration", "no-history-finding", &[]);
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
            transition_sites: Vec::new(),
        };

        let build_result = build_concentration_reports(
            &root,
            &file_paths,
            &complexity,
            &config,
            Some(&semantic),
            None,
        );
        let reports = build_result.reports;
        let findings = build_concentration_findings(&reports, 10);

        assert_eq!(reports.len(), 1);
        assert!(reports[0].score_0_10000 >= 3000);
        assert_eq!(findings.len(), 1);

        let _ = std::fs::remove_dir_all(root);
    }
}
