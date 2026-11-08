use lexgraph::app::model::{Document, PngSize};
use lexgraph::app::output::{
    ExportFormat, describe_error, export_bytes, export_rgba, parse_hex_color, rgb_to_hex,
};
use lexgraph::app::palette::{CATEGORIES, PaletteItem};
use lexgraph::app::plot::{compile, eval_constant, fit_view};
use lexgraph::app::storage::{FileKind, Session, decode, encode};
use lexgraph::render::{Rgb, Viewport};
use std::path::Path;

fn doc_with(sources: &[&str]) -> Document {
    let mut doc = Document::empty();
    for s in sources {
        doc.add_entry(s);
    }
    doc.sync_params();
    doc
}

#[test]
fn new_documents_start_with_automatic_range() {
    assert!(Document::empty().auto_fit);
}

#[test]
fn fit_keeps_x_and_scales_y_for_function_curves() {
    let mut doc = doc_with(&["x^2"]);
    doc.viewport = Viewport {
        x_min: -2.0,
        x_max: 2.0,
        y_min: -100.0,
        y_max: 100.0,
    };
    let fitted = fit_view(&doc, &compile(&doc));
    assert_eq!((fitted.x_min, fitted.x_max), (-2.0, 2.0));
    assert!(fitted.y_max > 3.0 && fitted.y_max < 6.0, "{fitted:?}");
    assert!(fitted.y_min < 0.5);
}

#[test]
fn fit_expands_x_to_contain_closed_curves() {
    let doc = doc_with(&["(30\\cos t, 30\\sin t)"]);
    let fitted = fit_view(&doc, &compile(&doc));
    assert!(fitted.x_max >= 30.0 && fitted.x_min <= -30.0, "{fitted:?}");
    assert!(fitted.y_max >= 25.0 && fitted.y_min <= -25.0, "{fitted:?}");
}

#[test]
fn fit_without_curves_keeps_the_view() {
    let doc = doc_with(&["\\frac{"]);
    assert_eq!(fit_view(&doc, &compile(&doc)), doc.viewport);
}

#[test]
fn constants_can_use_tex() {
    assert_eq!(eval_constant("2.5"), Some(2.5));
    assert!((eval_constant("2\\pi").unwrap() - std::f64::consts::TAU).abs() < 1e-12);
    assert_eq!(eval_constant("-1"), Some(-1.0));
    assert_eq!(eval_constant("x"), None);
    assert_eq!(eval_constant(""), None);
    assert_eq!(eval_constant("\\frac{1}{0}"), None);
}

#[test]
fn palette_puts_sin_and_log_up_front() {
    let first = &CATEGORIES[0];
    assert_eq!(first.items[0].insert, "\\sin ");
    assert_eq!(first.items[1].insert, "\\log ");
}

#[test]
fn palette_has_several_categories() {
    assert!(CATEGORIES.len() >= 4);
    let names: Vec<_> = CATEGORIES.iter().map(|c| c.name).collect();
    assert!(names.contains(&"ギリシャ文字"));
}

#[test]
fn every_palette_snippet_leaves_the_cursor_inside_it() {
    for category in CATEGORIES {
        for PaletteItem {
            insert,
            cursor_back,
            label,
        } in category.items
        {
            assert!(*cursor_back <= insert.chars().count(), "{label}");
        }
    }
}

#[test]
fn filled_palette_snippets_parse() {
    use lexgraph::core::parse_expression;
    for category in CATEGORIES {
        for item in category.items {
            let left = if item.needs_left_operand() { "x" } else { "" };
            let right = if item.needs_argument() { "x" } else { "" };
            let filled = format!("{left}{}{right}", item.insert.replace("{}", "{x}"));
            assert!(parse_expression(&filled).is_ok(), "{filled}");
        }
    }
}

#[test]
fn palette_cursor_lands_inside_the_first_hole() {
    for category in CATEGORIES {
        for item in category.items {
            if !item.insert.contains("{}") {
                continue;
            }
            let chars: Vec<char> = item.insert.chars().collect();
            let at = chars.len() - item.cursor_back;
            let before: String = chars[..at].iter().collect();
            assert!(before.ends_with('{'), "{}", item.label);
            assert!(!before[..before.len() - 1].contains("{}"), "{}", item.label);
        }
    }
}

#[test]
fn errors_are_described_with_position_and_fragment() {
    let source = "x+\\foo";
    let err = lexgraph::core::parse(source).unwrap_err();
    let text = describe_error(source, &err);
    assert!(text.starts_with("3文字目「\\foo」"), "{text}");
    assert!(text.contains("未対応"), "{text}");
}

#[test]
fn errors_at_the_end_say_so() {
    let source = "あ+";
    let err = lexgraph::core::parse_expression(source).unwrap_err();
    assert!(describe_error(source, &err).starts_with("1文字目"));
    let source = "x+";
    let err = lexgraph::core::parse(source).unwrap_err();
    assert!(describe_error(source, &err).starts_with("式の末尾"));
}

#[test]
fn hex_colors_round_trip() {
    assert_eq!(parse_hex_color("#ff8000"), Some(Rgb(255, 128, 0)));
    assert_eq!(parse_hex_color("FF8000"), Some(Rgb(255, 128, 0)));
    assert_eq!(parse_hex_color("#ff80"), None);
    assert_eq!(parse_hex_color("#gg8000"), None);
    assert_eq!(rgb_to_hex(Rgb(255, 128, 0)), "#ff8000");
}

#[test]
fn export_format_follows_the_extension() {
    assert_eq!(
        ExportFormat::from_path(Path::new("a.PNG")),
        Some(ExportFormat::Png)
    );
    assert_eq!(
        ExportFormat::from_path(Path::new("a.svg")),
        Some(ExportFormat::Svg)
    );
    assert_eq!(
        ExportFormat::from_path(Path::new("a.pdf")),
        Some(ExportFormat::Pdf)
    );
    assert_eq!(ExportFormat::from_path(Path::new("a.txt")), None);
    assert_eq!(ExportFormat::from_path(Path::new("a")), None);
}

#[test]
fn export_produces_each_format() {
    let doc = doc_with(&["x^2"]);
    let compiled = compile(&doc);
    let png = export_bytes(
        &doc,
        &compiled,
        ExportFormat::Png,
        false,
        PngSize::default(),
    )
    .unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let pdf = export_bytes(
        &doc,
        &compiled,
        ExportFormat::Pdf,
        false,
        PngSize::default(),
    )
    .unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let svg = export_bytes(
        &doc,
        &compiled,
        ExportFormat::Svg,
        false,
        PngSize::default(),
    )
    .unwrap();
    assert!(String::from_utf8(svg).unwrap().contains("rgb(255,255,255)"));
}

#[test]
fn transparent_export_has_no_background_rect() {
    let doc = doc_with(&["x^2"]);
    let compiled = compile(&doc);
    let svg = export_bytes(&doc, &compiled, ExportFormat::Svg, true, PngSize::default()).unwrap();
    assert!(
        !String::from_utf8(svg)
            .unwrap()
            .contains(r#"<rect x="0" y="0""#)
    );
}

#[test]
fn session_round_trips_with_path_and_pane_width() {
    let session = Session {
        document: doc_with(&["\\sin x"]),
        path: Some("C:\\work\\a.tgf".into()),
        pane_ratio: 0.4,
    };
    let bytes = encode(FileKind::Session, &session).unwrap();
    let back: Session = decode(FileKind::Session, &bytes).unwrap();
    assert_eq!(back, session);
}

fn png_dimensions(png: &[u8]) -> (u32, u32) {
    let number = |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
    (number(16), number(20))
}

#[test]
fn png_sizes_are_the_three_offered_and_default_to_1600() {
    assert_eq!(PngSize::default().pixels(), (1600, 1000));
    let all: Vec<_> = PngSize::ALL.iter().map(|s| s.pixels()).collect();
    assert_eq!(all, vec![(800, 500), (1600, 1000), (3200, 2000)]);
    assert_eq!(PngSize::default().to_string(), "1600×1000");
}

#[test]
fn png_export_uses_the_chosen_size() {
    let doc = doc_with(&["x^2"]);
    let compiled = compile(&doc);
    for size in PngSize::ALL {
        let png = export_bytes(&doc, &compiled, ExportFormat::Png, false, size).unwrap();
        assert_eq!(png_dimensions(&png), size.pixels());
    }
}

#[test]
fn clipboard_image_uses_the_chosen_size() {
    let doc = doc_with(&["x^2"]);
    let image = export_rgba(&doc, &compile(&doc), false, PngSize::Large).unwrap();
    assert_eq!((image.width, image.height), (3200, 2000));
    assert_eq!(image.pixels.len(), 3200 * 2000 * 4);
}

#[test]
fn svg_export_ignores_the_png_size() {
    let doc = doc_with(&["x^2"]);
    let compiled = compile(&doc);
    let small = export_bytes(&doc, &compiled, ExportFormat::Svg, false, PngSize::Small).unwrap();
    let large = export_bytes(&doc, &compiled, ExportFormat::Svg, false, PngSize::Large).unwrap();
    assert_eq!(small, large);
}
