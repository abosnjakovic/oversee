//! Centralised colour palette for oversee.
//!
//! Single source of truth for every colour used in the UI. Trail variants
//! provide phosphor-style age fading for the timeline waveform.

use ratatui::style::Color;

pub const TRAIL_TIERS: usize = 4;

pub struct Theme {
    pub cpu: Color,
    pub gpu: Color,
    pub mem: Color,

    /// Index 0 = newest (brightest), TRAIL_TIERS - 1 = oldest (faintest).
    pub cpu_trail: [Color; TRAIL_TIERS],
    pub gpu_trail: [Color; TRAIL_TIERS],
    pub mem_trail: [Color; TRAIL_TIERS],

    pub accent_warn: Color,
    pub accent_crit: Color,

    pub fg: Color,
    pub fg_dim: Color,
    pub fg_faint: Color,

    pub grid: Color,
    pub separator: Color,
    pub cursor: Color,
}

pub const THEME: Theme = Theme {
    cpu: Color::Cyan,
    gpu: Color::Magenta,
    mem: Color::Green,

    cpu_trail: [
        Color::Cyan,
        Color::Rgb(80, 180, 200),
        Color::Rgb(50, 110, 130),
        Color::Rgb(35, 70, 85),
    ],
    gpu_trail: [
        Color::Magenta,
        Color::Rgb(180, 80, 180),
        Color::Rgb(110, 50, 110),
        Color::Rgb(70, 35, 70),
    ],
    mem_trail: [
        Color::Green,
        Color::Rgb(80, 180, 100),
        Color::Rgb(45, 110, 60),
        Color::Rgb(30, 70, 40),
    ],

    accent_warn: Color::Yellow,
    accent_crit: Color::Red,

    fg: Color::White,
    fg_dim: Color::Gray,
    fg_faint: Color::DarkGray,

    grid: Color::Rgb(50, 50, 60),
    separator: Color::DarkGray,
    cursor: Color::White,
};

/// Map a column position to a trail tier.
/// `col` is 0..total, where 0 is the leftmost (oldest) column.
/// Returns 0..TRAIL_TIERS where 0 is the brightest (newest).
pub fn trail_tier(col: usize, total: usize) -> usize {
    if total <= 1 {
        return 0;
    }
    let from_right = total.saturating_sub(col + 1);
    (from_right * TRAIL_TIERS / total).min(TRAIL_TIERS - 1)
}
