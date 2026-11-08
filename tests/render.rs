use lexgraph::core::parse;
use lexgraph::render::{
    CurveSpec, LineStyle, Marker, MarkerKind, PlotSpec, Rgb, SceneTheme, Viewport, auto_fit_y,
    build_scene, format_tick, nice_ticks, sample_formula, to_pdf, to_png, to_png_sized, to_rgba,
    to_rgba_sized, to_svg,
};
use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, PI};

fn view() -> Viewport {
    Viewport {
        x_min: -10.0,
        x_max: 10.0,
        y_min: -10.0,
        y_max: 10.0,
    }
}

fn sample(src: &str, vp: Viewport) -> Vec<Vec<(f64, f64)>> {
    let formula = parse(src).unwrap();
    sample_formula(&formula, &BTreeMap::new(), &vp, (0.0, 2.0 * PI), 800)
}

#[test]
fn continuous_curve_is_one_polyline() {
    assert_eq!(sample("x^2", view()).len(), 1);
    assert_eq!(sample("\\sin x", view()).len(), 1);
}

#[test]
fn steep_continuous_curve_is_not_split() {
    let vp = Viewport {
        x_min: -100.0,
        x_max: 100.0,
        y_min: -10.0,
        y_max: 10.0,
    };
    assert_eq!(sample("x^3", vp).len(), 1);
}

#[test]
fn pole_splits_the_curve() {
    assert_eq!(sample("\\frac{1}{x}", view()).len(), 2);
}

#[test]
fn tangent_segments_never_cross_an_asymptote() {
    let lines = sample("\\tan x", view());
    assert_eq!(lines.len(), 7, "tan has 6 poles in [-10, 10]");
    for line in &lines {
        let lo = line.first().unwrap().0;
        let hi = line.last().unwrap().0;
        let cell = |x: f64| ((x + FRAC_PI_2) / PI).floor();
        assert_eq!(cell(lo + 1e-3), cell(hi - 1e-3), "{lo}..{hi}");
    }
}

#[test]
fn step_function_splits_at_each_jump() {
    let vp = Viewport {
        x_min: -2.5,
        x_max: 2.5,
        y_min: -5.0,
        y_max: 5.0,
    };
    assert_eq!(sample("\\lfloor x\\rfloor", vp).len(), 6);
}

#[test]
fn domain_edge_is_located_precisely() {
    let lines = sample("\\sqrt{x}", view());
    assert_eq!(lines.len(), 1);
    assert!(lines[0][0].0.abs() < 1e-3, "starts at {}", lines[0][0].0);
}

#[test]
fn nowhere_defined_curve_has_no_lines() {
    assert!(sample("\\ln(-x^2-1)", view()).is_empty());
}

#[test]
fn polar_circle_has_constant_radius() {
    let lines = sample("r=2", view());
    assert_eq!(lines.len(), 1);
    for (x, y) in &lines[0] {
        assert!(((x * x + y * y).sqrt() - 2.0).abs() < 1e-9);
    }
}

#[test]
fn parametric_uses_given_range() {
    let lines = sample("(\\cos t, \\sin t)", view());
    let first = lines[0][0];
    let last = *lines[0].last().unwrap();
    assert!((first.0 - 1.0).abs() < 1e-9 && first.1.abs() < 1e-9);
    assert!((last.0 - 1.0).abs() < 1e-9 && last.1.abs() < 1e-9);
}

#[test]
fn params_affect_sampling() {
    let formula = parse("a").unwrap();
    let params = BTreeMap::from([("a".to_string(), 3.0)]);
    let lines = sample_formula(&formula, &params, &view(), (0.0, 1.0), 100);
    assert!((lines[0][0].1 - 3.0).abs() < 1e-12);
}

#[test]
fn ticks_use_round_steps() {
    assert_eq!(
        nice_ticks(0.0, 10.0, 5),
        vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]
    );
    assert_eq!(nice_ticks(-1.0, 1.0, 4), vec![-1.0, -0.5, 0.0, 0.5, 1.0]);
    assert!(nice_ticks(1.0, 1.0, 5).is_empty());
}

#[test]
fn tick_labels_are_tidy() {
    assert_eq!(format_tick(0.30000000000000004), "0.3");
    assert_eq!(format_tick(-2.0), "-2");
    assert_eq!(format_tick(0.0), "0");
    assert_eq!(format_tick(1e7), "1e7");
}

#[test]
fn zoom_keeps_the_anchor_point_fixed() {
    let zoomed = view().zoom_at(5.0, -5.0, 0.5);
    assert_eq!(zoomed.x_min, -2.5);
    assert_eq!(zoomed.x_max, 7.5);
    assert_eq!(zoomed.y_min, -7.5);
    assert_eq!(zoomed.y_max, 2.5);
}

#[test]
fn zoom_refuses_to_collapse_the_view() {
    let mut vp = view();
    for _ in 0..500 {
        vp = vp.zoom_at(0.0, 0.0, 0.5);
    }
    assert!(vp.x_max > vp.x_min);
    assert!(vp.y_max > vp.y_min);
}

#[test]
fn pan_shifts_both_axes() {
    let moved = view().panned(1.0, -2.0);
    assert_eq!((moved.x_min, moved.x_max), (-9.0, 11.0));
    assert_eq!((moved.y_min, moved.y_max), (-12.0, 8.0));
}

#[test]
fn auto_fit_ignores_extreme_spikes() {
    let lines = sample("\\tan x", view());
    let (lo, hi) = auto_fit_y(&lines).unwrap();
    assert!(hi < 200.0 && lo > -200.0, "{lo}..{hi}");
    assert!(hi > 0.0 && lo < 0.0);
    assert!(auto_fit_y(&[]).is_none());
}

fn spec(background: Option<Rgb>) -> PlotSpec {
    PlotSpec {
        viewport: view(),
        curves: vec![CurveSpec {
            label: "y=x".into(),
            color: Rgb(200, 30, 30),
            width: 2.0,
            style: LineStyle::Dashed,
            lines: vec![vec![(-10.0, -10.0), (10.0, 10.0)]],
            fill: Some(vec![(0.0, 0.0), (1.0, 1.0), (1.0, 0.0)]),
        }],
        markers: vec![Marker {
            x: 0.0,
            y: 0.0,
            kind: MarkerKind::Root,
            color: Rgb(200, 30, 30),
            label: Some("(0, 0)".into()),
        }],
        show_grid: true,
        show_ticks: true,
        show_axis_labels: true,
        show_legend: true,
        theme: SceneTheme::light(background),
    }
}

#[test]
fn svg_contains_the_styled_curve_and_legend() {
    let svg = to_svg(&build_scene(&spec(Some(Rgb(255, 255, 255))), 400.0, 300.0));
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("stroke-dasharray"));
    assert!(svg.contains("rgb(200,30,30)"));
    assert!(svg.contains("y=x"));
    assert!(svg.contains("rgb(255,255,255)"));
    assert!(svg.contains("<polygon"));
}

#[test]
fn svg_escapes_markup_in_labels() {
    let mut s = spec(None);
    s.curves[0].label = "a<b&c".into();
    let svg = to_svg(&build_scene(&s, 400.0, 300.0));
    assert!(svg.contains("a&lt;b&amp;c"));
}

#[test]
fn overlapping_marker_labels_are_dropped_but_dots_stay() {
    let mut s = spec(None);
    let at = |x: f64, label: &str| Marker {
        x,
        y: 0.0,
        kind: MarkerKind::Root,
        color: Rgb(1, 2, 3),
        label: Some(label.into()),
    };
    s.markers = vec![at(0.0, "first"), at(0.02, "second"), at(5.0, "third")];
    let svg = to_svg(&build_scene(&s, 400.0, 300.0));
    assert!(svg.contains("first"));
    assert!(!svg.contains("second"));
    assert!(svg.contains("third"));
    assert_eq!(svg.matches("<circle").count(), 3);
}

#[test]
fn legend_can_be_hidden() {
    let mut s = spec(None);
    s.show_legend = false;
    assert!(!to_svg(&build_scene(&s, 400.0, 300.0)).contains("y=x"));
}

#[test]
fn grid_toggle_changes_output() {
    let mut s = spec(None);
    let with_grid = to_svg(&build_scene(&s, 400.0, 300.0));
    s.show_grid = false;
    assert!(to_svg(&build_scene(&s, 400.0, 300.0)).len() < with_grid.len());
}

#[test]
fn png_background_is_white_or_transparent() {
    let white = to_rgba(&build_scene(&spec(Some(Rgb(255, 255, 255))), 100.0, 80.0)).unwrap();
    assert_eq!((white.width, white.height), (100, 80));
    assert_eq!(&white.pixels[0..4], &[255, 255, 255, 255]);

    let clear = to_rgba(&build_scene(&spec(None), 100.0, 80.0)).unwrap();
    assert_eq!(clear.pixels[3], 0);
}

#[test]
fn png_rendering_draws_the_curve() {
    let image = to_rgba(&build_scene(&spec(Some(Rgb(255, 255, 255))), 200.0, 200.0)).unwrap();
    let reddish = image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 150 && p[1] < 90 && p[2] < 90)
        .count();
    assert!(reddish > 50, "only {reddish} curve pixels");
}

#[test]
fn png_and_pdf_have_valid_signatures() {
    let scene = build_scene(&spec(Some(Rgb(255, 255, 255))), 200.0, 150.0);
    assert!(to_png(&scene).unwrap().starts_with(b"\x89PNG"));
    assert!(to_pdf(&scene).unwrap().starts_with(b"%PDF"));
}

#[test]
fn sized_raster_scales_the_layout_instead_of_cropping_it() {
    let scene = build_scene(&spec(Some(Rgb(255, 255, 255))), 400.0, 300.0);
    let ink = |image: &lexgraph::render::RgbaImage| {
        let total = image.pixels.len() / 4;
        let marked = image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] < 200)
            .count();
        marked as f64 / total as f64
    };
    let normal = to_rgba(&scene).unwrap();
    let double = to_rgba_sized(&scene, 800, 600).unwrap();
    assert_eq!((double.width, double.height), (800, 600));
    let (a, b) = (ink(&normal), ink(&double));
    assert!(b > a * 0.6 && b < a * 1.4, "{a} vs {b}");
}

#[test]
fn sized_png_has_the_requested_dimensions() {
    let scene = build_scene(&spec(Some(Rgb(255, 255, 255))), 400.0, 300.0);
    let png = to_png_sized(&scene, 200, 150).unwrap();
    assert_eq!(
        u32::from_be_bytes([png[16], png[17], png[18], png[19]]),
        200
    );
    assert_eq!(
        u32::from_be_bytes([png[20], png[21], png[22], png[23]]),
        150
    );
}
