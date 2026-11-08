use lexgraph::app::history::History;
use lexgraph::app::model::PALETTE;
use lexgraph::app::theme::{
    ACCENT, ACCENT_INK, CANVAS, ERROR, FIELD_EDGE, GROUND, INK, INK_MUTED, RAISED, SHELL, SURFACE,
    contrast,
};
use lexgraph::core::{ParseErrorKind, parse};
use lexgraph::render::{Rgb, SceneTheme};

const BACKGROUNDS: [(&str, Rgb); 5] = [
    ("canvas", CANVAS),
    ("ground", GROUND),
    ("shell", SHELL),
    ("surface", SURFACE),
    ("raised", RAISED),
];

fn luminance(c: Rgb) -> f64 {
    contrast(c, Rgb(0, 0, 0)) * 0.05 - 0.05
}

#[test]
fn contrast_matches_known_values() {
    assert!((contrast(Rgb(255, 255, 255), Rgb(0, 0, 0)) - 21.0).abs() < 0.01);
    assert!((contrast(Rgb(0, 0, 0), Rgb(255, 255, 255)) - 21.0).abs() < 0.01);
    assert!((contrast(Rgb(119, 119, 119), Rgb(255, 255, 255)) - 4.48).abs() < 0.02);
}

#[test]
fn text_is_readable_on_every_background() {
    for (name, background) in BACKGROUNDS {
        assert!(contrast(INK, background) >= 7.0, "ink on {name}");
        assert!(contrast(INK_MUTED, background) >= 4.5, "muted on {name}");
        assert!(contrast(ERROR, background) >= 4.5, "error on {name}");
    }
}

#[test]
fn field_edges_are_visible() {
    for (name, background) in BACKGROUNDS {
        assert!(contrast(FIELD_EDGE, background) >= 3.0, "edge on {name}");
    }
}

#[test]
fn accent_works_as_a_fill_and_as_a_mark() {
    assert!(contrast(ACCENT_INK, ACCENT) >= 4.5);
    for (name, background) in BACKGROUNDS {
        assert!(contrast(ACCENT, background) >= 3.0, "accent on {name}");
    }
}

#[test]
fn layers_get_lighter_as_they_rise() {
    let ladder = [CANVAS, GROUND, SURFACE, RAISED];
    for pair in ladder.windows(2) {
        assert!(luminance(pair[1]) > luminance(pair[0]), "{pair:?}");
    }
}

#[test]
fn no_pure_black_or_white() {
    for color in [CANVAS, GROUND, SHELL, SURFACE, RAISED, INK, INK_MUTED] {
        assert_ne!(color, Rgb(0, 0, 0));
        assert_ne!(color, Rgb(255, 255, 255));
    }
}

#[test]
fn curve_colors_read_on_screen_and_on_white_exports() {
    for color in PALETTE {
        assert!(contrast(color, CANVAS) >= 3.0, "{color:?} on canvas");
        assert!(
            contrast(color, Rgb(255, 255, 255)) >= 3.0,
            "{color:?} on white"
        );
    }
}

#[test]
fn graph_uses_the_app_canvas_color() {
    assert_eq!(SceneTheme::dark().background, Some(CANVAS));
}

#[test]
fn graph_text_is_readable() {
    let theme = SceneTheme::dark();
    assert!(contrast(theme.text, CANVAS) >= 4.5);
    assert!(contrast(theme.text, theme.legend_background) >= 4.5);
    assert!(contrast(theme.axis, CANVAS) >= 3.0);
}

#[test]
fn history_reports_what_is_available() {
    let mut h = History::new(10);
    assert!(!h.can_undo() && !h.can_redo());
    h.record(1);
    assert!(h.can_undo() && !h.can_redo());
    assert_eq!(h.undo(2), Some(1));
    assert!(!h.can_undo() && h.can_redo());
}

#[test]
fn every_input_error_says_what_to_do() {
    let cases = [
        ("\\foo", "綴り"),
        ("(x", "閉じ"),
        ("x)", "開き"),
        ("x+", "数"),
        ("\\frac{1}", "{}"),
        ("y=y", "別の"),
        ("x=2", "y="),
    ];
    for (source, hint) in cases {
        let message = parse(source).unwrap_err().to_string();
        assert!(message.contains(hint), "{source}: {message}");
        assert!(
            message.contains('。'),
            "{source}: needs a reason and an action: {message}"
        );
    }
    assert!(
        ParseErrorKind::BadTuple
            .to_string()
            .contains("(x(t), y(t))")
    );
}
