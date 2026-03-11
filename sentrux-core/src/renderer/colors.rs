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

/// Language → color mapping via O(1) match.
pub fn language_color(lang: &str) -> Color32 {
    let (r, g, b) = match lang {
        "python"     => (65, 105, 145),
        "javascript" | "jsx" => (175, 165, 85),
        "typescript" | "tsx" => (60, 110, 168),
        "rust"       => (175, 135, 110),
        "go"         => (55, 140, 165),
        "c"          => (90, 95, 100),
        "cpp"        => (55, 90, 140),
        "java"       => (150, 110, 55),
        "ruby"       => (160, 65, 60),
        "csharp"     => (105, 60, 120),
        "php"        => (105, 110, 150),
        "bash"       => (110, 160, 80),
        "html"       => (175, 80, 55),
        "css"        => (85, 70, 120),
        "scss"       => (155, 95, 125),
        "swift"      => (180, 80, 60),
        "kotlin"     => (135, 105, 190),
        "lua"        => (50, 55, 120),
        "scala"      => (155, 60, 75),
        "elixir"     => (100, 75, 120),
        "haskell"    => (90, 80, 125),
        "zig"        => (180, 135, 60),
        "r"          => (50, 120, 175),
        "dockerfile" => (60, 80, 90),
        "ocaml"      => (180, 110, 45),
        "json"       => (60, 65, 70),
        "toml"       => (130, 75, 50),
        "yaml"       => (155, 50, 55),
        "markdown"   => (50, 70, 135),
        _            => (80, 85, 90),
    };
    Color32::from_rgb(r, g, b)
}

/// Git status → color
pub fn git_color(gs: &str) -> Color32 {
    match gs {
        "A" => Color32::from_rgb(72, 191, 145),
        "M" => Color32::from_rgb(255, 193, 7),
        "MM" => Color32::from_rgb(255, 152, 0),
        "D" => Color32::from_rgb(244, 67, 54),
        "R" => Color32::from_rgb(156, 39, 176),
        "?" => Color32::from_rgb(120, 120, 120),
        _ => Color32::from_rgb(70, 70, 70),
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

