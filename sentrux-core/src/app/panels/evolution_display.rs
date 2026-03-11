//! Evolution metrics display — churn, bus factor, hotspots, change coupling.
//!
//! Renders the EvolutionReport data in the metrics panel.
//! Covers 4 of the 6 missing UI capabilities:
//! 1. Evolution metrics (churn grade, hotspots)
//! 2. Bus factor (single-author risk)
//! 3. Change coupling (co-change pairs)
//! 4. Hottest files (temporal hotspots)

use crate::metrics::evo::EvolutionReport;
use crate::core::settings::ThemeConfig;
use super::ui_helpers::dim_grade_color;

/// Draw the evolution section in the metrics panel.
pub(crate) fn draw_evolution_section(ui: &mut egui::Ui, report: &EvolutionReport, tc: &ThemeConfig) {
    let row_h = 13.0;
    let font = egui::FontId::monospace(9.0);

    // Section header
    ui.label(
        egui::RichText::new("EVOLUTION")
            .monospace()
            .size(9.0)
            .color(tc.section_label),
    );
    ui.add_space(2.0);

    // Overall grade
    let grade_color = dim_grade_color(report.evolution_grade, tc);
    let (grade_rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(grade_rect.left() + 4.0, grade_rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("Grade: {}", report.evolution_grade),
        egui::FontId::monospace(11.0),
        grade_color,
    );
    ui.add_space(2.0);

    // Metrics rows: (label, value, grade, tooltip)
    let commits_tooltip = format!("Total commits analyzed (last {} days)", report.lookback_days);
    let metrics: Vec<(&str, String, char, &str)> = vec![
        ("churn", format!("{} files", report.churn.len()), report.churn_grade,
         "Top-10% files' share of total churn | lower=better"),
        ("bus factor", format!("{:.0}% solo", report.single_author_ratio * 100.0), report.bus_factor_grade,
         "Ricca 2011 | ratio of single-author files | lower=better"),
        ("commits", format!("{}", report.commits_analyzed), '-',
         commits_tooltip.as_str()),
    ];

    for (label, value, grade, tooltip) in &metrics {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
        let cy = rect.center().y;
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, cy),
            egui::Align2::LEFT_CENTER,
            label,
            font.clone(),
            tc.text_secondary,
        );
        if *grade != '-' {
            let g_color = dim_grade_color(*grade, tc);
            ui.painter().text(
                egui::pos2(rect.right() - 4.0, cy),
                egui::Align2::RIGHT_CENTER,
                format!("{}", grade),
                font.clone(),
                g_color,
            );
            ui.painter().text(
                egui::pos2(rect.right() - 24.0, cy),
                egui::Align2::RIGHT_CENTER,
                value,
                font.clone(),
                tc.text_secondary,
            );
        } else {
            ui.painter().text(
                egui::pos2(rect.right() - 4.0, cy),
                egui::Align2::RIGHT_CENTER,
                value,
                font.clone(),
                tc.text_secondary,
            );
        }
        if resp.hovered() {
            resp.on_hover_text(egui::RichText::new(*tooltip).monospace().size(9.0));
        }
    }

    // Temporal hotspots
    draw_hotspots(ui, report, tc, row_h);

    // Change coupling pairs
    draw_coupling(ui, report, tc, row_h);

    // Bus factor details (risky single-author files)
    draw_bus_factor(ui, report, tc, row_h);
}

fn draw_hotspots(ui: &mut egui::Ui, report: &EvolutionReport, _tc: &ThemeConfig, row_h: f32) {
    if report.hotspots.is_empty() { return; }
    let color = egui::Color32::from_rgb(200, 140, 80);
    ui.add_space(3.0);
    ui.label(egui::RichText::new("HOTSPOTS (churn x complexity)").monospace().size(8.0).color(color));
    for hs in report.hotspots.iter().take(5) {
        let name = hs.file.rsplit('/').next().unwrap_or(&hs.file);
        let text = format!("  {} ({}x{}={})", name, hs.churn_count, hs.max_complexity, hs.risk_score);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
        if resp.hovered() {
            resp.on_hover_text(egui::RichText::new(&hs.file).monospace().size(10.0));
        }
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER, &text,
            egui::FontId::monospace(8.0), color);
    }
    if report.hotspots.len() > 5 {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("  +{} more", report.hotspots.len() - 5),
            egui::FontId::monospace(8.0), egui::Color32::from_rgb(140, 140, 140));
    }
}

fn draw_coupling(ui: &mut egui::Ui, report: &EvolutionReport, _tc: &ThemeConfig, row_h: f32) {
    if report.coupling_pairs.is_empty() { return; }
    let color = egui::Color32::from_rgb(140, 180, 200);
    ui.add_space(3.0);
    ui.label(egui::RichText::new("CHANGE COUPLING (co-change)").monospace().size(8.0).color(color));
    for pair in report.coupling_pairs.iter().take(5) {
        let a = pair.file_a.rsplit('/').next().unwrap_or(&pair.file_a);
        let b = pair.file_b.rsplit('/').next().unwrap_or(&pair.file_b);
        let text = format!("  {} <> {} ({} J:{:.0}%)", a, b, pair.co_change_count, pair.coupling_strength * 100.0);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
        if resp.hovered() {
            resp.on_hover_text(
                egui::RichText::new(format!("{} <> {}", pair.file_a, pair.file_b)).monospace().size(10.0)
            );
        }
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER, &text,
            egui::FontId::monospace(8.0), color);
    }
    if report.coupling_pairs.len() > 5 {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("  +{} more pairs", report.coupling_pairs.len() - 5),
            egui::FontId::monospace(8.0), egui::Color32::from_rgb(140, 140, 140));
    }
}

fn draw_bus_factor(ui: &mut egui::Ui, report: &EvolutionReport, _tc: &ThemeConfig, row_h: f32) {
    // Show files with only 1 author (highest bus factor risk)
    let mut single_author_files: Vec<(&str, &str)> = report.authors.iter()
        .filter(|(_, info)| info.author_count == 1)
        .map(|(path, info)| (path.as_str(), info.primary_author.as_str()))
        .collect();
    // Sort by path for deterministic display order (HashMap iteration is non-deterministic)
    single_author_files.sort_unstable_by_key(|(path, _)| *path);
    single_author_files.truncate(5);

    if single_author_files.is_empty() { return; }

    let color = egui::Color32::from_rgb(200, 160, 200);
    ui.add_space(3.0);
    ui.label(egui::RichText::new("BUS FACTOR RISK (single author)").monospace().size(8.0).color(color));
    for (path, author) in single_author_files.iter().take(5) {
        let name = path.rsplit('/').next().unwrap_or(path);
        let text = format!("  {} ({})", name, author);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
        if resp.hovered() {
            resp.on_hover_text(egui::RichText::new(*path).monospace().size(10.0));
        }
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER, &text,
            egui::FontId::monospace(8.0), color);
    }
}
