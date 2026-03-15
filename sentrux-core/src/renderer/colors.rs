//! Color mapping functions for all ColorMode variants.
//!
//! Maps file attributes (language, git status, age, blast radius, churn)
//! to `Color32` values. Palette is desaturated for readability — colors
//! distinguish categories without competing with text labels or edges.

use egui::Color32;

/// Blast radius → red gradient. High blast = bright red (dangerous to change),
/// low blast = dim green (safe to change).
pub fn blast_radius_color(radius: u32, max_radius: u32) -> Color32 {
    if max_radius == 0 {
        return Color32::from_rgb(60, 140, 80); // all safe
    }
    let t = (radius as f32 / max_radius as f32).min(1.0);
    // green(safe) → yellow → red(dangerous)
    let r = (60.0 + t * 195.0) as u8;
    let g = (160.0 - t * 120.0) as u8;
    let b = (80.0 - t * 50.0) as u8;
    Color32::from_rgb(r, g, b)
}

/// Language → color from plugin profile.
/// Each plugin declares its color in plugin.toml: `color_rgb = [65, 105, 145]`.
/// Languages without plugins (json, toml, yaml, etc.) get default gray.
pub fn language_color(lang: &str) -> Color32 {
    let rgb = crate::analysis::lang_registry::profile(lang).color_rgb;
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Git status → color (VS Code dark theme palette — muted, professional)
pub fn git_color(gs: &str) -> Color32 {
    match gs {
        "A"  => Color32::from_rgb(115, 201, 145), // muted green — new/added
        "M"  => Color32::from_rgb(103, 150, 230), // muted blue — modified
        "MM" => Color32::from_rgb(130, 160, 240), // slightly brighter blue — staged+working
        "D"  => Color32::from_rgb(224, 108, 117), // muted red — deleted
        "R"  => Color32::from_rgb(209, 154, 102), // muted amber — renamed
        "?"  => Color32::from_rgb(115, 201, 145), // same green — untracked (new to git)
        _    => Color32::from_rgb(70, 70, 70),
    }
}

/// Exec depth → blue gradient. Depth 0 (entry points) = bright/prominent,
/// deeper dependencies = dimmer. Inverted t so shallow = visually important.
pub fn exec_depth_color(depth: u32) -> Color32 {
    let t = 1.0 - (depth as f32 / 8.0).min(1.0); // invert: 0=bright, 8+=dim
    let r = (40.0 + t * 60.0) as u8;
    let g = (60.0 + t * 100.0) as u8;
    let b = (180.0 + t * 75.0) as u8;
    Color32::from_rgb(r, g, b)
}

