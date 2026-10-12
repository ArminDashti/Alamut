//! Modern, colorful palette shared by every view.

use ratatui::style::{Color, Modifier, Style};

pub const CYAN: Color = Color::Rgb(34, 211, 238);
pub const MAGENTA: Color = Color::Rgb(232, 121, 249);
pub const AMBER: Color = Color::Rgb(251, 191, 36);
pub const LIME: Color = Color::Rgb(163, 230, 53);
pub const SKY: Color = Color::Rgb(56, 189, 248);
pub const ROSE: Color = Color::Rgb(251, 113, 133);
pub const VIOLET: Color = Color::Rgb(167, 139, 250);
pub const GREEN: Color = Color::Rgb(74, 222, 128);
pub const SLATE: Color = Color::Rgb(148, 163, 184);
pub const FG: Color = Color::Rgb(226, 232, 240);
pub const DIM: Color = Color::Rgb(100, 116, 139);
pub const HEADER_BG: Color = Color::Rgb(30, 41, 59);
pub const SEL_BG: Color = Color::Rgb(51, 65, 85);

/// Accent color per tab index (cycles through the palette).
pub const ACCENTS: [Color; 7] = [CYAN, MAGENTA, AMBER, LIME, SKY, ROSE, VIOLET];

pub fn accent(idx: usize) -> Color {
    ACCENTS[idx % ACCENTS.len()]
}

/// Gradient spans for the app wordmark (one color per character).
pub fn wordmark(text: &str) -> Vec<Style> {
    let palette = [CYAN, SKY, MAGENTA, VIOLET, AMBER, LIME];
    text.chars()
        .enumerate()
        .map(|(i, _)| Style::default().fg(palette[i % palette.len()]).add_modifier(Modifier::BOLD))
        .collect()
}
