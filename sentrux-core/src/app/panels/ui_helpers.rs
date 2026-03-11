//! Shared UI drawing helpers to eliminate duplication across display panels.
//!
//! Provides core primitives:
//! - `dim_grade_color`: color for grade letters
//! - `draw_flagged_list`: titled item list with hover tooltips and "+N more" overflow

use crate::core::settings::ThemeConfig;

/// Color for a per-dimension grade letter.
pub(crate) fn dim_grade_color(g: char, tc: &ThemeConfig) -> egui::Color32 {
    match g {
        'A' => egui::Color32::from_rgb(100, 200, 100),
        'B' => egui::Color32::from_rgb(160, 200, 100),
        'C' => egui::Color32::from_rgb(200, 180, 80),
        'D' => egui::Color32::from_rgb(200, 120, 60),
        'F' => egui::Color32::from_rgb(200, 80, 80),
        _ => tc.text_secondary,
    }
}

#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_flagged_list<T, F, H>(
    ui: &mut egui::Ui,
    title: &str,
    items: &[T],
    color: egui::Color32,
    row_h: f32,
    max_items: usize,
    format_fn: F,
    hover_fn: H,
) where
    F: Fn(&T) -> String,
    H: Fn(&T) -> String,
{
    if items.is_empty() {
        return;
    }
    ui.add_space(3.0);
    ui.label(egui::RichText::new(title).monospace().size(8.0).color(color));
    for item in items.iter().take(max_items) {
        let text = format_fn(item);
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
        if resp.hovered() {
            resp.on_hover_text(egui::RichText::new(hover_fn(item)).monospace().size(10.0));
        }
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &text,
            egui::FontId::monospace(8.0),
            color,
        );
    }
    let remaining = items.len().saturating_sub(max_items);
    if remaining > 0 {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
        ui.painter().text(
            egui::pos2(rect.left() + 4.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("  +{} more", remaining),
            egui::FontId::monospace(8.0),
            egui::Color32::from_rgb(140, 140, 140),
        );
    }
}

