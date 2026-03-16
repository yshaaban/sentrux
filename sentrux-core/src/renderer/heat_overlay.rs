//! Heat overlay rendering — ripple effects and warm glow on recently changed files.
//!
//! Ripples are expanding border animations triggered by file saves. Heat glow
//! is a semi-transparent warm tint proportional to the file's decayed heat value.
//! Also renders the activity trail: fading highlights on recently changed files.

use super::{ColorMode, RectKind, RenderData, heat, RenderContext};
use egui::{Color32, CornerRadius, Stroke, StrokeKind};
use std::collections::HashSet;

/// Draw ripple effect for a single file rect (expanding sharp border, pixel style).
fn draw_ripple(painter: &egui::Painter, screen_rect: egui::Rect, progress: f64) {
    let ripple_color = heat::ripple_color(progress);
    let expand = ((progress as f32) * 4.0).floor().max(1.0);
    let expanded = screen_rect.expand(expand);
    let w = if progress < 0.5 { 2.0 } else { 1.0 };
    painter.rect_stroke(expanded, CornerRadius::ZERO, Stroke::new(w, ripple_color), StrokeKind::Outside);
}

/// Draw heat glow overlay for a single file rect.
/// Color depends on event kind: teal mint for create, honey amber for modify.
/// Alpha adapts to ensure visibility against any block background color.
fn draw_heat_glow(painter: &egui::Painter, screen_rect: egui::Rect, heat_value: f64, kind: heat::EventKind, bg_color: Color32) {
    if heat_value <= 0.05 { return; }
    let base = heat::heat_color_for_kind(heat_value, kind);

    // Adaptive alpha: increase intensity when glow color is similar to background.
    // Compute hue distance between glow and bg — similar hues need more alpha to be visible.
    let glow_lum = 0.2126 * base.r() as f32 / 255.0 + 0.7152 * base.g() as f32 / 255.0 + 0.0722 * base.b() as f32 / 255.0;
    let bg_lum = 0.2126 * bg_color.r() as f32 / 255.0 + 0.7152 * bg_color.g() as f32 / 255.0 + 0.0722 * bg_color.b() as f32 / 255.0;
    let lum_contrast = (glow_lum - bg_lum).abs();

    // Low contrast (similar colors) → boost alpha. High contrast → normal alpha.
    let base_alpha = (heat_value * 16.0).min(80.0);
    let boost = if lum_contrast < 0.15 { 1.8 } else if lum_contrast < 0.3 { 1.3 } else { 1.0 };
    let alpha = (base_alpha * boost).min(140.0) as u8;

    let glow = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha);
    painter.rect_filled(screen_rect, CornerRadius::ZERO, glow);
}

/// Draw a single activity trail dot for a file — color matches event kind.
fn draw_trail_dot(painter: &egui::Painter, screen_rect: egui::Rect, heat_value: f64, dot_radius: f32, kind: heat::EventKind) {
    let alpha = (heat_value * 16.0).min(80.0) as u8;
    if alpha <= 10 { return; }
    let base = heat::heat_color_for_kind(heat_value, kind);
    let dot_pos = egui::pos2(screen_rect.left() + 2.0, screen_rect.top() + 2.0);
    let dot_rect = egui::Rect::from_min_size(dot_pos, egui::vec2(dot_radius * 2.0, dot_radius * 2.0));
    painter.rect_filled(dot_rect, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha));
}

/// Draw activity trail: fading glow markers for recent changes.
/// Builds a HashMap from file rects for O(1) lookups instead of O(N*M) linear scan.
fn draw_activity_trail(
    painter: &egui::Painter, canvas_origin: egui::Pos2, rd: &RenderData, ctx: &RenderContext,
) {
    if ctx.heat.trail.is_empty() { return; }
    let vp = &ctx.viewport;

    // Build path → rect lookup for O(1) access per trail entry.
    let rect_map: std::collections::HashMap<&str, &crate::layout::types::LayoutRectSlim> = rd.rects
        .iter()
        .filter(|r| r.kind == RectKind::File)
        .map(|r| (r.path.as_str(), r))
        .collect();

    let mut trail_seen: HashSet<&str> = HashSet::new();
    for (path, _time) in &ctx.heat.trail {
        if !trail_seen.insert(path.as_str()) { continue; }
        let r = match rect_map.get(path.as_str()) {
            Some(r) => *r,
            None => continue,
        };
        if !vp.is_visible(r.x, r.y, r.w, r.h) { continue; }
        let screen_rect = vp.world_to_screen_rect(r.x, r.y, r.w, r.h, canvas_origin);
        if screen_rect.width() < 2.0 { continue; }
        let h = ctx.heat.get_heat(path, ctx.frame_instant, ctx.settings.heat_half_life);
        let kind = ctx.heat.get_kind(path);
        draw_trail_dot(painter, screen_rect, h, ctx.settings.trail_dot_radius, kind);
    }
}

/// Draw ripple and heat glow for a single file rect.
fn draw_file_heat(
    painter: &egui::Painter,
    screen_rect: egui::Rect,
    path: &str,
    bg_color: Color32,
    ctx: &RenderContext,
) {
    if let Some(progress) = ctx.heat.get_ripple(path, ctx.frame_instant, ctx.settings.ripple_duration) {
        draw_ripple(painter, screen_rect, progress);
    }
    if ctx.color_mode != ColorMode::Heat {
        let h = ctx.heat.get_heat(path, ctx.frame_instant, ctx.settings.heat_half_life);
        let kind = ctx.heat.get_kind(path);
        draw_heat_glow(painter, screen_rect, h, kind, bg_color);
    }
}

/// Draw heat overlays: ripple glow on recently changed files, activity trail fading highlights.
pub(crate) fn draw_heat_overlays(
    painter: &egui::Painter,
    clip_rect: egui::Rect,
    rd: &RenderData,
    ctx: &RenderContext,
) {
    let canvas_origin = clip_rect.min;
    let vp = &ctx.viewport;

    // Only iterate all rects when there are active ripples or heat glow to draw.
    // When color mode is Heat, heat glow is already rendered via file color,
    // so we only need this loop for ripples.
    let need_ripple_pass = ctx.heat.has_any_ripples();
    let need_glow_pass = ctx.color_mode != ColorMode::Heat && ctx.heat.is_active();
    if need_ripple_pass || need_glow_pass {
        for r in &rd.rects {
            if r.kind != RectKind::File || !vp.is_visible(r.x, r.y, r.w, r.h) { continue; }
            let screen_rect = vp.world_to_screen_rect(r.x, r.y, r.w, r.h, canvas_origin);
            if screen_rect.width() < 1.0 || screen_rect.height() < 1.0 { continue; }
            // Default dark background — correct for most blocks in dark theme.
            // Heat glow alpha auto-boosts when contrast is low.
            let bg_color = Color32::from_rgb(20, 25, 35);
            draw_file_heat(painter, screen_rect, &r.path, bg_color, ctx);
        }
    }

    draw_activity_trail(painter, canvas_origin, rd, ctx);
}
