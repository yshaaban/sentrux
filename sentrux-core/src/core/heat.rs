//! File heat tracking — exponential-decay activity heatmap.
//!
//! Tracks per-file "heat" that spikes on each file save and decays
//! exponentially over time. Used for the Heat color mode, ripple animations,
//! and the activity trail display. All timing is based on `Instant` (monotonic).

use egui::Color32;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

/// Maximum heat value a file can accumulate (caps exponential stacking).
const MAX_HEAT: f64 = 5.0;

/// Maximum number of trail entries retained (prevents unbounded growth).
const MAX_TRAIL: usize = 500;

/// File system event kind — determines heat color.
#[derive(Clone, Copy, PartialEq)]
pub enum EventKind {
    /// New file created — teal mint glow
    Create,
    /// Existing file modified — honey amber glow
    Modify,
}

/// Per-file heat entry
struct HeatEntry {
    /// Initial heat level at last_change time (accumulated from rapid edits)
    initial_heat: f64,
    /// Current decayed heat level
    heat: f64,
    /// When the last change was recorded
    last_change: Instant,
    /// When the ripple animation started (None = no active ripple)
    ripple_start: Option<Instant>,
    /// What kind of event triggered this heat
    kind: EventKind,
}

/// Tracks per-file heat (recent edit activity) with exponential decay.
pub struct HeatTracker {
    entries: HashMap<String, HeatEntry>,
    /// Recent change trail: (path, time) — kept for 30s. Uses VecDeque for O(1) front removal.
    pub trail: VecDeque<(String, Instant)>,
}

/// Heat config passed from Settings at call sites.
/// Avoids coupling HeatTracker to the Settings struct directly.
pub struct HeatConfig {
    /// Exponential decay half-life in seconds for heat values
    pub half_life: f64,
    /// Duration in seconds for the ripple border animation
    pub ripple_duration: f64,
    /// Maximum age in seconds before trail entries are pruned
    pub trail_max_age: f64,
}

impl HeatTracker {
    /// Create an empty heat tracker with no entries.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            trail: VecDeque::new(),
        }
    }

    /// Record a file change — boosts heat to 1.0 and starts a ripple.
    /// `kind` determines the glow color (teal mint for create, amber for modify).
    pub fn record_change(&mut self, path: &str, cfg: &HeatConfig) {
        self.record_change_with_kind(path, cfg, EventKind::Modify);
    }

    /// Record a file change with specific event kind.
    pub fn record_change_with_kind(&mut self, path: &str, cfg: &HeatConfig, kind: EventKind) {
        let now = Instant::now();
        let entry = self.entries.entry(path.to_string()).or_insert(HeatEntry {
            initial_heat: 0.0,
            heat: 0.0,
            last_change: now,
            ripple_start: None,
            kind,
        });
        entry.kind = kind;
        // Compute live-decayed heat before accumulating, to avoid using the
        // stale cached value from the last tick(). [ref:4f5a9de5]
        let dt = now.duration_since(entry.last_change).as_secs_f64();
        let hl = cfg.half_life.max(0.001);
        let live_heat = entry.initial_heat * (-std::f64::consts::LN_2 * dt / hl).exp();
        entry.initial_heat = (live_heat + 1.0).min(MAX_HEAT);
        entry.heat = entry.initial_heat;
        entry.last_change = now;
        entry.ripple_start = Some(now);

        self.trail.push_back((path.to_string(), now));
        while self.trail.len() > MAX_TRAIL {
            self.trail.pop_front();
        }
    }

    /// Tick all entries — apply exponential decay, prune cold entries, trim trail.
    pub fn tick(&mut self, cfg: &HeatConfig) {
        let now = Instant::now();
        let ln2 = std::f64::consts::LN_2;

        self.entries.retain(|_, e| {
            let dt = now.duration_since(e.last_change).as_secs_f64();
            let hl = cfg.half_life.max(0.001);
            e.heat = e.initial_heat * (-ln2 * dt / hl).exp();

            // Clear expired ripple
            if let Some(rs) = e.ripple_start {
                if now.duration_since(rs).as_secs_f64() > cfg.ripple_duration {
                    e.ripple_start = None;
                }
            }

            e.heat > 0.01 // prune near-zero
        });

        // Trim old trail entries — pop from front since trail is time-ordered
        // (push_back with monotonically increasing timestamps). O(k) where k is
        // the number of expired entries, instead of O(n) full scan. [M5 fix]
        while let Some((_p, t)) = self.trail.front() {
            if now.duration_since(*t).as_secs_f64() >= cfg.trail_max_age {
                self.trail.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get current heat for a file (0.0–5.0 raw, 0.0 if not tracked).
    /// Computes live decay at query time for smooth animation (not stale
    /// cached value from last tick). [ref:4f5a9de5]
    /// Takes `now` to avoid calling Instant::now() per-file per-frame
    /// (previously ~2000 syscalls/frame for 1000 visible files).
    #[inline]
    pub fn get_heat(&self, path: &str, now: Instant, half_life: f64) -> f64 {
        self.entries.get(path).map_or(0.0, |e| {
            let dt = now.duration_since(e.last_change).as_secs_f64();
            let hl = half_life.max(0.001);
            e.initial_heat * (-std::f64::consts::LN_2 * dt / hl).exp()
        })
    }

    /// Get ripple progress for a file (0.0–1.0, None if no active ripple).
    /// Takes `now` for frame-consistent rendering (same timestamp for all files).
    #[inline]
    pub fn get_ripple(&self, path: &str, now: Instant, ripple_duration: f64) -> Option<f64> {
        self.entries.get(path).and_then(|e| {
            e.ripple_start.and_then(|rs| {
                let t = now.duration_since(rs).as_secs_f64() / ripple_duration.max(0.001);
                if t >= 1.0 {
                    None // Expired — don't issue draw calls for phantom shapes
                } else {
                    Some(t)
                }
            })
        })
    }

    /// Get event kind for a file (Create or Modify). Returns Modify as default.
    #[inline]
    pub fn get_kind(&self, path: &str) -> EventKind {
        self.entries.get(path).map_or(EventKind::Modify, |e| e.kind)
    }

    /// True if any file has visually significant heat (above rendering threshold).
    /// Prevents infinite repaint loops from near-zero heat values.
    pub fn is_active(&self) -> bool {
        self.entries.values().any(|e| e.heat > 0.02)
    }

    /// True if any file has an active (unexpired) ripple animation.
    pub fn has_any_ripples(&self) -> bool {
        self.entries.values().any(|e| e.ripple_start.is_some())
    }

    /// Get all files with heat > threshold, sorted by heat descending.
    /// Uses live exponential decay (same as get_heat) for consistency. [ref:4f5a9de5]
    pub fn hot_files(&self, threshold: f64, now: Instant, half_life: f64) -> Vec<(&str, f64)> {
        let ln2 = std::f64::consts::LN_2;
        let mut v: Vec<(&str, f64)> = self
            .entries
            .iter()
            .map(|(k, e)| {
                let dt = now.duration_since(e.last_change).as_secs_f64();
                let hl = half_life.max(0.001);
                let live_heat = e.initial_heat * (-ln2 * dt / hl).exp();
                (k.as_str(), live_heat)
            })
            .filter(|(_, h)| *h > threshold)
            .collect();
        v.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        v
    }
}

/// Heat value + event kind → color.
///
/// Create: teal mint (#56d4a4) — fresh, cool, "something new appeared"
/// Modify: honey amber (#d4a856) → warm gold → deep amber as heat builds
///
/// Palette A (Refined Terminal): 120°+ hue separation, all WCAG AA on dark bg.
pub fn heat_color(heat: f64) -> Color32 {
    heat_color_for_kind(heat, EventKind::Modify)
}

/// Heat color with event kind differentiation.
pub fn heat_color_for_kind(heat: f64, kind: EventKind) -> Color32 {
    let t = (heat / MAX_HEAT).clamp(0.0, 1.0) as f32;
    match kind {
        EventKind::Create => {
            // Teal mint: #56d4a4 at full heat → fades toward dark
            let r = (20.0 + t * 66.0) as u8;   // 20 → 86
            let g = (30.0 + t * 182.0) as u8;   // 30 → 212
            let b = (25.0 + t * 139.0) as u8;   // 25 → 164
            Color32::from_rgb(r, g, b)
        }
        EventKind::Modify => {
            // Honey amber: #d4a856 at low → warm gold → deep amber at high
            if t < 0.5 {
                let u = t * 2.0;
                let r = (40.0 + u * 172.0) as u8;  // 40 → 212
                let g = (40.0 + u * 128.0) as u8;  // 40 → 168
                let b = (30.0 + u * 56.0) as u8;   // 30 → 86
                Color32::from_rgb(r, g, b)
            } else {
                let u = (t - 0.5) * 2.0;
                let r = (212.0 + u * 43.0) as u8;  // 212 → 255
                let g = (168.0 - u * 48.0) as u8;  // 168 → 120
                let b = (86.0 - u * 56.0) as u8;   // 86 → 30
                Color32::from_rgb(r, g, b)
            }
        }
    }
}

/// Ripple glow color with alpha based on ripple progress (fades out).
pub fn ripple_color(progress: f64) -> Color32 {
    let alpha = ((1.0 - progress) * 180.0) as u8;
    Color32::from_rgba_unmultiplied(255, 200, 80, alpha)
}
