//! Centralised colour palette for oversee.
//!
//! Single source of truth for every colour used in the UI.

use ratatui::style::Color;

pub struct Theme {
    pub cpu: Color,
    pub gpu: Color,
    pub mem: Color,

    pub accent_warn: Color,
    pub accent_crit: Color,

    pub fg: Color,
    pub fg_dim: Color,
    pub fg_faint: Color,

    pub grid: Color,
    pub separator: Color,
}

pub const THEME: Theme = Theme {
    cpu: Color::Cyan,
    gpu: Color::Magenta,
    mem: Color::Green,

    accent_warn: Color::Yellow,
    accent_crit: Color::Red,

    fg: Color::White,
    fg_dim: Color::Gray,
    fg_faint: Color::DarkGray,

    grid: Color::Rgb(50, 50, 60),
    separator: Color::DarkGray,
};
