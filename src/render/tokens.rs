//! Color tokens shared by the graph and the user interface.
//!
//! The dark ground is a tinted ink blue, never pure black. Each layer that rises
//! above the canvas is lighter, so depth reads without shadows.
use super::scene::Rgb;

/// Behind the graph, the lowest layer.
pub const CANVAS: Rgb = Rgb(14, 19, 27);
/// Behind the expression list and graph controls.
pub const GROUND: Rgb = Rgb(20, 26, 36);
/// The toolbar and status line, which frame the work areas.
pub const SHELL: Rgb = Rgb(18, 34, 52);
/// Expression cards and dialogs.
pub const SURFACE: Rgb = Rgb(28, 36, 48);
/// Buttons and other controls on a surface.
pub const RAISED: Rgb = Rgb(38, 48, 63);
pub const INK: Rgb = Rgb(232, 237, 244);
pub const INK_MUTED: Rgb = Rgb(170, 181, 196);
/// The edge of an input field, which must stay visible on every layer.
pub const FIELD_EDGE: Rgb = Rgb(112, 126, 145);
/// Selection and focus. Curve colors stay reserved for curves.
pub const ACCENT: Rgb = Rgb(240, 178, 74);
pub const ACCENT_INK: Rgb = Rgb(28, 22, 8);
pub const ERROR: Rgb = Rgb(255, 138, 128);
pub const GRID: Rgb = Rgb(38, 47, 60);
pub const AXIS: Rgb = Rgb(150, 162, 178);

fn linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Rgb) -> f64 {
    0.2126 * linear(c.0) + 0.7152 * linear(c.1) + 0.0722 * linear(c.2)
}

/// WCAG contrast ratio between two colors, from 1 to 21.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}
