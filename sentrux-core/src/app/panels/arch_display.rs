//! Architecture report display — renders levelization, distance, blast radius.
//!
//! Shows the architecture grade and per-dimension metrics (upward violations,
//! distance from main sequence, blast radius, attack surface) with grades.

use crate::metrics::arch::ArchReport;
use crate::core::settings::ThemeConfig;
use super::ui_helpers::dim_grade_color;

/// Draw the architecture report section in the activity panel.
/// Shows arch grade, levelization, distance from main sequence, blast radius,
/// attack surface, and top upward violations.
pub(crate) fn draw_arch_section(ui: &mut egui::Ui, arch: &ArchReport, tc: &ThemeConfig) {
    let row_h = 13.0;
    let font = egui::FontId::monospace(9.0);

    // Section header + overall grade
    draw_arch_header(ui, arch, tc);

    // Dimension rows: label  value  [grade]
    draw_arch_dimension_rows(ui, arch, tc, row_h, &font);

    // Info rows (no grade): max level + violations count
    draw_arch_info_rows(ui, arch, tc, row_h, &font);

    // Detailed sub-sections
    draw_arch_violations(ui, arch, tc, row_h);
    draw_arch_blast_radius(ui, arch, row_h);
    draw_arch_distance(ui, arch, row_h);
}

/// Draw section header and overall architecture grade.
fn draw_arch_header(ui: &mut egui::Ui, arch: &ArchReport, tc: &ThemeConfig) {
    ui.label(
        egui::RichText::new("ARCHITECTURE")
            .monospace().size(9.0).color(tc.section_label),
    );
    ui.add_space(2.0);

    let grade_color = dim_grade_color(arch.arch_grade, tc);
    let (grade_rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 18.0), egui::Sense::hover(),
    );
    ui.painter().text(
        egui::pos2(grade_rect.left() + 4.0, grade_rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("Grade: {}", arch.arch_grade),
        egui::FontId::monospace(11.0), grade_color,
    );
    ui.add_space(2.0);
}

/// Draw the four architecture dimension rows with grades.
fn draw_arch_dimension_rows(ui: &mut egui::Ui, arch: &ArchReport, tc: &ThemeConfig, row_h: f32, font: &egui::FontId) {
    let arch_metrics: Vec<(&str, String, char)> = vec![
        ("levelization", format!("{:.0}% upward", arch.upward_ratio * 100.0), arch.levelization_grade),
        ("distance", format!("{:.2} avg", arch.avg_distance), arch.distance_grade),
        ("blast radius", format!("{} max", arch.max_blast_radius), arch.blast_grade),
        ("attack surface", format!("{:.0}%", arch.attack_surface_ratio * 100.0), arch.surface_grade),
    ];
    for (label, value, grade) in &arch_metrics {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_h), egui::Sense::hover(),
        );
        let cy = rect.center().y;
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, cy), egui::Align2::LEFT_CENTER,
            label, font.clone(), tc.text_secondary,
        );
        ui.painter().text(
            egui::pos2(rect.right() - 4.0, cy), egui::Align2::RIGHT_CENTER,
            format!("{}", grade), font.clone(), dim_grade_color(*grade, tc),
        );
        ui.painter().text(
            egui::pos2(rect.right() - 24.0, cy), egui::Align2::RIGHT_CENTER,
            value, font.clone(), tc.text_secondary,
        );
    }
}

/// Draw info-only rows: max level and violations count (no grade letter).
fn draw_arch_info_rows(ui: &mut egui::Ui, arch: &ArchReport, tc: &ThemeConfig, row_h: f32, font: &egui::FontId) {
    // Max level row
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_h), egui::Sense::hover(),
    );
    ui.painter().text(
        egui::pos2(rect.left() + 4.0, rect.center().y), egui::Align2::LEFT_CENTER,
        "max level", font.clone(), tc.text_secondary,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 4.0, rect.center().y), egui::Align2::RIGHT_CENTER,
        format!("{}", arch.max_level), font.clone(), tc.text_secondary,
    );

    // Violations count row
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_h), egui::Sense::hover(),
    );
    ui.painter().text(
        egui::pos2(rect.left() + 4.0, rect.center().y), egui::Align2::LEFT_CENTER,
        "violations", font.clone(), tc.text_secondary,
    );
    let v_count = arch.upward_violations.len();
    let v_color = if v_count == 0 {
        egui::Color32::from_rgb(100, 200, 100)
    } else {
        egui::Color32::from_rgb(200, 120, 60)
    };
    ui.painter().text(
        egui::pos2(rect.right() - 4.0, rect.center().y), egui::Align2::RIGHT_CENTER,
        format!("{}", v_count), font.clone(), v_color,
    );
}

/// Draw the top upward violations list (max 3 shown).
fn draw_arch_violations(ui: &mut egui::Ui, arch: &ArchReport, tc: &ThemeConfig, row_h: f32) {
    if arch.upward_violations.is_empty() { return; }
    ui.add_space(3.0);
    let warn_color = egui::Color32::from_rgb(200, 120, 60);
    ui.label(
        egui::RichText::new("UPWARD VIOLATIONS")
            .monospace().size(8.0).color(warn_color),
    );
    for v in arch.upward_violations.iter().take(2) {
        let from_name = v.from_file.rsplit('/').next().unwrap_or(&v.from_file);
        let to_name = v.to_file.rsplit('/').next().unwrap_or(&v.to_file);
        let text = format!("  L{} {} -> L{} {}", v.from_level, from_name, v.to_level, to_name);
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_h), egui::Sense::hover(),
        );
        if resp.hovered() {
            resp.on_hover_text(
                egui::RichText::new(format!("{} -> {}", v.from_file, v.to_file))
                    .monospace().size(10.0),
            );
        }
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER, &text,
            egui::FontId::monospace(8.0), warn_color,
        );
    }
    if arch.upward_violations.len() > 2 {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_h), egui::Sense::hover(),
        );
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("  +{} more", arch.upward_violations.len() - 2),
            egui::FontId::monospace(8.0), tc.text_secondary,
        );
    }
}

/// Draw the highest blast radius file.
fn draw_arch_blast_radius(ui: &mut egui::Ui, arch: &ArchReport, row_h: f32) {
    if arch.max_blast_file.is_empty() { return; }
    ui.add_space(3.0);
    let color = egui::Color32::from_rgb(200, 100, 100);
    ui.label(
        egui::RichText::new("HIGHEST BLAST RADIUS")
            .monospace().size(8.0).color(color),
    );
    let name = arch.max_blast_file.rsplit('/').next().unwrap_or(&arch.max_blast_file);
    let text = format!("  {} ({} files)", name, arch.max_blast_radius);
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_h), egui::Sense::hover(),
    );
    if resp.hovered() {
        resp.on_hover_text(
            egui::RichText::new(&arch.max_blast_file).monospace().size(10.0),
        );
    }
    ui.painter().text(
        egui::pos2(rect.left() + 4.0, rect.center().y),
        egui::Align2::LEFT_CENTER, &text,
        egui::FontId::monospace(8.0), color,
    );
}

/// Draw modules furthest from the main sequence (distance > 0.3, max 3).
fn draw_arch_distance(ui: &mut egui::Ui, arch: &ArchReport, row_h: f32) {
    let worst: Vec<_> = arch.distance_metrics.iter()
        .filter(|m| m.distance > 0.3)
        .take(2)
        .collect();
    if worst.is_empty() { return; }
    ui.add_space(3.0);
    let color = egui::Color32::from_rgb(180, 140, 200);
    ui.label(
        egui::RichText::new("OFF MAIN SEQUENCE")
            .monospace().size(8.0).color(color),
    );
    for m in &worst {
        let text = format!("  {} D:{:.2} A:{:.1} I:{:.1}",
            m.module, m.distance, m.abstractness, m.instability);
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_h), egui::Sense::hover(),
        );
        if resp.hovered() {
            resp.on_hover_text(egui::RichText::new(format!(
                "Module: {}\nAbstractness: {:.2} ({} abstract / {} total)\nInstability: {:.2} (fan-in:{} fan-out:{})\nDistance: {:.3}",
                m.module, m.abstractness, m.abstract_count, m.total_types,
                m.instability, m.fan_in, m.fan_out, m.distance
            )).monospace().size(10.0));
        }
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER, &text,
            egui::FontId::monospace(8.0), color,
        );
    }
}
