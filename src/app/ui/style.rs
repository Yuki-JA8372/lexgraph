//! Widget styles. Hover, press and disabled are each one visible step, and border
//! widths never change between states, so nothing shifts when a state changes.
use crate::app::theme::{
    ACCENT, ACCENT_INK, CANVAS, ERROR, FIELD_EDGE, GROUND, INK, INK_MUTED, RAISED, SHELL, SURFACE,
};
use crate::render::Rgb;
use crate::render::canvas::color;
use iced::widget::{button, container, pane_grid, text_editor, text_input};
use iced::{Background, Border, Color, Theme};

pub const RADIUS: f32 = 4.0;
pub const CARD_RADIUS: f32 = 6.0;

fn fill(c: Rgb) -> Option<Background> {
    Some(Background::Color(color(c)))
}

fn edge(c: Color) -> Border {
    Border {
        color: c,
        width: 1.0,
        radius: RADIUS.into(),
    }
}

fn dimmed(c: Rgb) -> Color {
    color(c).scale_alpha(0.45)
}

/// A quiet button that shows its shape on hover. For toolbars and tabs.
pub fn tool_button(_: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Active => (None, color(INK)),
        button::Status::Hovered => (fill(RAISED), color(INK)),
        button::Status::Pressed => (fill(SURFACE), color(INK)),
        button::Status::Disabled => (None, dimmed(INK_MUTED)),
    };
    button::Style {
        background,
        text_color,
        border: edge(Color::TRANSPARENT),
        ..Default::default()
    }
}

/// A button with a resting shape, for actions inside cards and dialogs.
pub fn control_button(_: &Theme, status: button::Status) -> button::Style {
    let (background, border, text_color) = match status {
        button::Status::Active => (fill(RAISED), Color::TRANSPARENT, color(INK)),
        button::Status::Hovered => (fill(RAISED), color(FIELD_EDGE), color(INK)),
        button::Status::Pressed => (fill(SURFACE), color(FIELD_EDGE), color(INK)),
        button::Status::Disabled => (fill(SURFACE), Color::TRANSPARENT, dimmed(INK_MUTED)),
    };
    button::Style {
        background,
        text_color,
        border: edge(border),
        ..Default::default()
    }
}

/// The one main action of a dialog, and the selected palette tab.
pub fn primary_button(_: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Pressed => color(ACCENT).scale_alpha(0.8),
        button::Status::Disabled => color(ACCENT).scale_alpha(0.4),
        _ => color(ACCENT),
    };
    let hovered = status == button::Status::Hovered;
    button::Style {
        background: Some(Background::Color(background)),
        text_color: color(ACCENT_INK),
        border: edge(if hovered {
            color(INK)
        } else {
            Color::TRANSPARENT
        }),
        ..Default::default()
    }
}

/// A square of a curve's color.
pub fn swatch_button(c: Rgb) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: fill(c),
        border: edge(if status == button::Status::Active {
            Color::TRANSPARENT
        } else {
            color(INK)
        }),
        ..Default::default()
    }
}

pub fn shell(_: &Theme) -> container::Style {
    container::Style {
        background: fill(SHELL),
        ..Default::default()
    }
}

pub fn ground(_: &Theme) -> container::Style {
    container::Style {
        background: fill(GROUND),
        ..Default::default()
    }
}

/// An expression card. The selected one is outlined; the size never changes.
pub fn card(active: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: fill(SURFACE),
        border: Border {
            color: if active {
                color(ACCENT)
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: CARD_RADIUS.into(),
        },
        ..Default::default()
    }
}

/// The bar on a card's edge that ties an expression to its curve.
pub fn curve_bar(c: Rgb, visible: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(if visible {
            color(c)
        } else {
            color(c).scale_alpha(0.3)
        })),
        border: Border {
            radius: 2.0.into(),
            ..Border::default()
        },
        ..Default::default()
    }
}

pub fn dialog(_: &Theme) -> container::Style {
    container::Style {
        background: fill(SURFACE),
        border: Border {
            color: color(FIELD_EDGE),
            width: 1.0,
            radius: CARD_RADIUS.into(),
        },
        ..Default::default()
    }
}

pub fn backdrop(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color(CANVAS).scale_alpha(0.75))),
        ..Default::default()
    }
}

pub fn tooltip_box(_: &Theme) -> container::Style {
    container::Style {
        background: fill(RAISED),
        text_color: Some(color(INK)),
        border: edge(color(FIELD_EDGE)),
        ..Default::default()
    }
}

fn field_edge_color(focused: bool, hovered: bool, invalid: bool) -> Color {
    if invalid {
        color(ERROR)
    } else if focused {
        color(ACCENT)
    } else if hovered {
        color(INK_MUTED)
    } else {
        color(FIELD_EDGE)
    }
}

/// The expression input. An invalid one keeps a red edge even while focused.
pub fn tex_editor(invalid: bool) -> impl Fn(&Theme, text_editor::Status) -> text_editor::Style {
    move |_, status| {
        let focused = matches!(status, text_editor::Status::Focused { .. });
        let hovered = status == text_editor::Status::Hovered;
        text_editor::Style {
            background: Background::Color(color(CANVAS)),
            border: edge(field_edge_color(focused, hovered, invalid)),
            placeholder: color(INK_MUTED),
            value: color(INK),
            selection: color(ACCENT).scale_alpha(0.35),
        }
    }
}

pub fn field(_: &Theme, status: text_input::Status) -> text_input::Style {
    let focused = matches!(status, text_input::Status::Focused { .. });
    let hovered = status == text_input::Status::Hovered;
    text_input::Style {
        background: Background::Color(color(CANVAS)),
        border: edge(field_edge_color(focused, hovered, false)),
        icon: color(INK_MUTED),
        placeholder: color(INK_MUTED),
        value: color(INK),
        selection: color(ACCENT).scale_alpha(0.35),
    }
}

/// Shows where the panes can be dragged apart.
pub fn panes(theme: &Theme) -> pane_grid::Style {
    let line = |width| pane_grid::Line {
        color: color(ACCENT),
        width,
    };
    pane_grid::Style {
        hovered_split: line(2.0),
        picked_split: line(3.0),
        ..pane_grid::default(theme)
    }
}
