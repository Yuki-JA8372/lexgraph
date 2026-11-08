use lexgraph::core::analysis::{
    ExtremumKind, find_extrema, find_intersections, find_roots, integrate,
};
use lexgraph::core::{Env, parse_expression};
use std::f64::consts::{FRAC_PI_2, PI};

fn func(src: &str) -> impl Fn(f64) -> f64 {
    let expr = parse_expression(src).unwrap();
    move |x| {
        expr.eval(&Env {
            var_name: "x".into(),
            var: x,
            params: Default::default(),
        })
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn roots_found_at_sign_changes() {
    let roots = find_roots(&func("x^2-4"), -5.0, 5.0, 800);
    assert_eq!(roots.len(), 2, "{roots:?}");
    assert!(near(roots[0], -2.0) && near(roots[1], 2.0));
}

#[test]
fn touching_root_is_found_without_sign_change() {
    let roots = find_roots(&func("x^2"), -3.0, 3.1, 800);
    assert_eq!(roots.len(), 1, "{roots:?}");
    assert!(roots[0].abs() < 1e-4);
}

#[test]
fn poles_are_not_roots() {
    assert!(find_roots(&func("\\frac{1}{x}"), -1.0, 1.1, 800).is_empty());
    let tan_roots = find_roots(&func("\\tan x"), -1.0, 2.0, 800);
    assert_eq!(tan_roots.len(), 1, "{tan_roots:?}");
    assert!(tan_roots[0].abs() < 1e-6);
}

#[test]
fn exact_sample_root_is_not_duplicated() {
    let roots = find_roots(&func("x"), -1.0, 1.0, 800);
    assert_eq!(roots.len(), 1, "{roots:?}");
}

#[test]
fn extrema_classified_as_max_and_min() {
    let found = find_extrema(&func("\\sin x"), 0.0, 2.0 * PI, 800);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].kind, ExtremumKind::Max);
    assert!((found[0].x - FRAC_PI_2).abs() < 1e-4);
    assert!(near(found[0].y, 1.0));
    assert_eq!(found[1].kind, ExtremumKind::Min);
}

#[test]
fn kink_is_an_extremum() {
    let found = find_extrema(&func("|x|"), -1.0, 1.1, 800);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].kind, ExtremumKind::Min);
    assert!(found[0].x.abs() < 1e-4);
}

#[test]
fn monotonic_flat_and_stepped_have_no_extrema() {
    assert!(find_extrema(&func("x^3+x"), -2.0, 2.0, 800).is_empty());
    assert!(find_extrema(&func("3"), -2.0, 2.0, 800).is_empty());
    assert!(find_extrema(&func("\\lfloor x\\rfloor"), -2.2, 2.3, 800).is_empty());
    assert!(find_extrema(&func("\\frac{1}{x}"), -1.0, 1.1, 800).is_empty());
}

#[test]
fn intersections_report_both_coordinates() {
    let f = func("x^2");
    let g = func("x+2");
    let points = find_intersections(&f, &g, -5.0, 5.0, 800);
    assert_eq!(points.len(), 2, "{points:?}");
    assert!(near(points[0].0, -1.0) && near(points[0].1, 1.0));
    assert!(near(points[1].0, 2.0) && near(points[1].1, 4.0));
}

#[test]
fn integrate_is_signed_and_accurate() {
    assert!(near(integrate(&func("x^2"), 0.0, 3.0), 9.0));
    assert!(near(integrate(&func("\\sin x"), 0.0, PI), 2.0));
    assert!(near(integrate(&func("x"), -2.0, 0.0), -2.0));
    assert!(near(integrate(&func("x"), 2.0, 0.0), -2.0));
}

#[test]
fn integrate_undefined_region_is_nan() {
    assert!(integrate(&func("\\ln x"), -1.0, 1.0).is_nan());
}
