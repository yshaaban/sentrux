//! Shared UI drawing helpers to eliminate duplication across display panels.
//!
//! Provides core primitives:
//! - `score_color`: continuous color from [0,1] score
//! - `draw_flagged_list`: titled item list with hover tooltips and "+N more" overflow

/// Continuous color from score ∈ [0, 1]. No grade boundaries.
/// 0.0 = red, 0.5 = yellow, 1.0 = green. Smooth gradient.
pub(crate) fn score_color(score: f64) -> egui::Color32 {
    let s = score.clamp(0.0, 1.0) as f32;
    if s < 0.5 {
        // Red → Yellow (0.0 → 0.5)
        let t = s * 2.0;
        egui::Color32::from_rgb(
            200,
            (80.0 + t * 120.0) as u8, // 80 → 200
            (80.0 - t * 20.0) as u8,  // 80 → 60
        )
    } else {
        // Yellow → Green (0.5 → 1.0)
        let t = (s - 0.5) * 2.0;
        egui::Color32::from_rgb(
            (200.0 - t * 100.0) as u8, // 200 → 100
            200,
            (60.0 + t * 40.0) as u8,   // 60 → 100
        )
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

