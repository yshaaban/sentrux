use super::path_roles::{annotate_structural_leverage, structural_presentation_class};
use super::scoring::{dead_private_cluster_score, signal_severity};
use super::utils::dedupe_strings_preserve_order;
use super::{
    FileFacts, StructuralDebtMetrics, StructuralDebtReport, StructuralSignalClass,
    StructuralTrustTier,
};
use crate::metrics::HealthReport;
use std::collections::BTreeMap;

pub(super) fn build_dead_private_code_cluster_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
) -> Vec<StructuralDebtReport> {
    let mut dead_by_file = BTreeMap::<String, Vec<_>>::new();
    for function in &health.dead_functions {
        dead_by_file
            .entry(function.file.clone())
            .or_default()
            .push(function.clone());
    }

    dead_by_file
        .into_iter()
        .filter_map(|(path, functions)| {
            let mut unique_functions = BTreeMap::new();
            for function in functions {
                unique_functions
                    .entry(function.func.clone())
                    .and_modify(|existing: &mut crate::metrics::FuncMetric| {
                        if function.value > existing.value {
                            *existing = function.clone();
                        }
                    })
                    .or_insert(function);
            }
            let functions = unique_functions.into_values().collect::<Vec<_>>();
            let dead_symbol_count = functions.len();
            let dead_line_count = functions
                .iter()
                .map(|function| function.value as usize)
                .sum::<usize>();
            if dead_symbol_count < 2 && dead_line_count < 40 {
                return None;
            }
            let facts = file_facts.get(&path)?;
            let score_0_10000 = dead_private_cluster_score(dead_symbol_count, dead_line_count);
            let function_names = functions
                .iter()
                .take(3)
                .map(|function| function.func.clone())
                .collect::<Vec<_>>();

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "dead_private_code_cluster".to_string(),
                trust_tier: StructuralTrustTier::Experimental,
                presentation_class: structural_presentation_class(
                    "dead_private_code_cluster",
                    &path,
                    StructuralTrustTier::Experimental,
                    &facts.role_tags,
                ),
                leverage_class: Default::default(),
                scope: path.clone(),
                signal_class: StructuralSignalClass::Watchpoint,
                signal_families: vec!["staleness".to_string(), "maintainability".to_string()],
                severity: signal_severity(score_0_10000),
                score_0_10000,
                summary: format!(
                    "File '{}' contains {} uncalled private functions totaling {} lines",
                    path, dead_symbol_count, dead_line_count
                ),
                impact: "Stale private code increases maintenance noise and can mislead future edits into reviving obsolete paths.".to_string(),
                files: vec![path.clone()],
                role_tags: facts.role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order(vec![
                    format!("dead private functions: {}", dead_symbol_count),
                    format!("dead private lines: {}", dead_line_count),
                    format!("sample dead functions: {}", function_names.join(", ")),
                    format!("total file lines: {}", facts.lines),
                ]),
                inspection_focus: vec![
                    "inspect whether the dead helpers should be deleted or intentionally reconnected".to_string(),
                    "inspect whether the file still reflects the supported control flow".to_string(),
                ],
                candidate_split_axes: Vec::new(),
                related_surfaces: Vec::new(),
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(facts.lines),
                    function_count: Some(facts.function_count),
                    dead_symbol_count: Some(dead_symbol_count),
                    dead_line_count: Some(dead_line_count),
                    max_complexity: Some(facts.max_complexity),
                    role_count: Some(facts.role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}
