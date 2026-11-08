//! The iced theme built from the shared color tokens.
pub use crate::render::tokens::*;

use crate::render::canvas::color;
use iced::Theme;
use iced::theme::Palette;

/// The application's dark theme.
pub fn app_theme() -> Theme {
    Theme::custom(
        "LexGraph",
        Palette {
            background: color(GROUND),
            text: color(INK),
            primary: color(ACCENT),
            success: color(ACCENT),
            warning: color(ACCENT),
            danger: color(ERROR),
        },
    )
}
