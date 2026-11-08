use lexgraph::core::{Env, Expr, FormulaKind, ParseErrorKind, parse, parse_expression};
use std::f64::consts::{E, PI};

fn env(var_name: &str, var: f64, params: &[(&str, f64)]) -> Env {
    Env {
        var_name: var_name.into(),
        var,
        params: params.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
    }
}

fn val(src: &str, x: f64) -> f64 {
    parse_expression(src).expect(src).eval(&env("x", x, &[]))
}

fn val_with(src: &str, x: f64, params: &[(&str, f64)]) -> f64 {
    parse_expression(src).expect(src).eval(&env("x", x, params))
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

#[test]
fn arithmetic_respects_precedence() {
    close(val("1+2*3", 0.0), 7.0);
    close(val("(1+2)\\cdot 3", 0.0), 9.0);
    close(val("7-2-1", 0.0), 4.0);
    close(val("8/2/2", 0.0), 2.0);
    close(val("6\\div 3\\times 2", 0.0), 4.0);
}

#[test]
fn unary_minus_binds_looser_than_power() {
    close(val("-x^2", 3.0), -9.0);
    close(val("2^{-1}", 0.0), 0.5);
    close(val("2^-1", 0.0), 0.5);
}

#[test]
fn power_is_right_associative() {
    close(val("2^{3^{2}}", 0.0), 512.0);
}

#[test]
fn superscript_takes_single_token_like_tex() {
    close(val_with("x^2y", 3.0, &[("y", 2.0)]), 18.0);
    // In TeX, 2^12 is 2^1 followed by 2, i.e. 2 * 2.
    close(val("2^12", 0.0), 4.0);
    close(val("2^{12}", 0.0), 4096.0);
}

#[test]
fn implicit_multiplication() {
    close(val("2x", 3.0), 6.0);
    close(val("2(x+1)", 3.0), 8.0);
    close(val("(x+1)(x-1)", 3.0), 8.0);
    close(val("2\\pi x", 1.0), 2.0 * PI);
    close(val("x\\sin x", 2.0), 2.0 * 2f64.sin());
}

#[test]
fn frac_and_sqrt() {
    close(val("\\frac{x}{2}", 3.0), 1.5);
    close(val("\\frac12", 0.0), 0.5);
    close(val("\\sqrt{x}", 9.0), 3.0);
    close(val("\\sqrt[3]{x}", 8.0), 2.0);
    close(val("\\sqrt4", 0.0), 2.0);
}

#[test]
fn trig_and_log_functions() {
    close(val("\\sin x", 1.0), 1f64.sin());
    close(val("\\cos(x)", 1.0), 1f64.cos());
    close(val("\\tan x", 1.0), 1f64.tan());
    close(val("\\arctan x", 1.0), 1f64.atan());
    close(val("\\sinh x", 1.0), 1f64.sinh());
    close(val("\\ln x", E), 1.0);
    close(val("\\log x", 100.0), 2.0);
    close(val("\\log_{2} x", 8.0), 3.0);
    close(val("\\log_2 8", 0.0), 3.0);
    close(val("\\exp(x)", 1.0), E);
    close(val("e^{x}", 1.0), E);
}

#[test]
fn function_argument_scope_without_parens() {
    close(val("\\sin 2x", 1.0), 2f64.sin());
    close(val("\\sin x^2", 2.0), 4f64.sin());
    close(val("\\sin x\\cos x", 1.0), 1f64.sin() * 1f64.cos());
    close(val("\\sin x+1", 1.0), 1f64.sin() + 1.0);
    close(val_with("\\sin(x)a", 1.0, &[("a", 2.0)]), 2.0 * 1f64.sin());
}

#[test]
fn function_powers() {
    close(val("\\sin^2 x", 1.0), 1f64.sin().powi(2));
    close(val("\\sin^{-1} x", 0.5), 0.5f64.asin());
    close(val("\\sin(x)^2", 1.0), 1f64.sin().powi(2));
}

#[test]
fn absolute_floor_ceil_minmax() {
    close(val("|x|", -3.0), 3.0);
    close(val("\\left|x\\right|", -3.0), 3.0);
    close(val("|x||x|", -2.0), 4.0);
    close(val("2|x-1|", 0.0), 2.0);
    close(val("\\lfloor x\\rfloor", 1.5), 1.0);
    close(val("\\lceil x\\rceil", 1.5), 2.0);
    close(val("\\max(x,2)", 1.0), 2.0);
    close(val("\\min(x,2)", 1.0), 1.0);
}

#[test]
fn left_right_and_spacing_commands_are_ignored() {
    close(val("\\left(x+1\\right)\\,\\cdot\\;2", 1.0), 4.0);
    close(val("{x}+{1}", 1.0), 2.0);
}

#[test]
fn bare_expression_is_cartesian_with_detected_params() {
    let f = parse("a x^2 + b").unwrap();
    assert!(matches!(f.kind, FormulaKind::Cartesian(_)));
    assert_eq!(f.params, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn lhs_forms_select_formula_kind() {
    assert!(matches!(
        parse("y=x").unwrap().kind,
        FormulaKind::Cartesian(_)
    ));
    assert!(matches!(
        parse("f(x)=x").unwrap().kind,
        FormulaKind::Cartesian(_)
    ));
    assert!(matches!(
        parse("r=2\\theta").unwrap().kind,
        FormulaKind::Polar(_)
    ));
    assert!(matches!(
        parse("(\\cos t, \\sin t)").unwrap().kind,
        FormulaKind::Parametric { .. }
    ));
}

#[test]
fn main_variables_are_not_params() {
    assert_eq!(parse("r=a\\theta").unwrap().params, vec!["a".to_string()]);
    assert_eq!(parse("(a t, t)").unwrap().params, vec!["a".to_string()]);
    assert_eq!(
        parse("\\theta x").unwrap().params,
        vec!["\\theta".to_string()]
    );
    assert!(parse("e^{\\pi x}").unwrap().params.is_empty());
}

#[test]
fn subscripted_param_is_one_name() {
    let f = parse("a_1 x + a_{2}").unwrap();
    assert_eq!(f.params, vec!["a_1".to_string(), "a_2".to_string()]);
}

#[test]
fn params_are_read_from_env() {
    let e = parse_expression("a x + b").unwrap();
    close(e.eval(&env("x", 2.0, &[("a", 3.0), ("b", 1.0)])), 7.0);
}

#[test]
fn missing_param_evaluates_to_nan() {
    assert!(
        parse_expression("a")
            .unwrap()
            .eval(&env("x", 0.0, &[]))
            .is_nan()
    );
}

#[test]
fn errors_report_location_and_reason() {
    let err = parse("x+\\foo").unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::UnknownCommand("\\foo".into()));
    assert_eq!(err.span, 2..6);

    let err = parse("(x+1").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::Unclosed(_)));
    assert_eq!(err.span.start, 0);

    let err = parse("x+").unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::ExpectedValue);
    assert_eq!(err.span.start, 2);

    let err = parse("x)").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::UnmatchedClose(_)));
    assert_eq!(err.span, 1..2);

    assert_eq!(parse("  ").unwrap_err().kind, ParseErrorKind::Empty);
    assert!(matches!(
        parse("x#").unwrap_err().kind,
        ParseErrorKind::UnexpectedChar('#')
    ));
    assert!(matches!(
        parse("\\frac{1}").unwrap_err().kind,
        ParseErrorKind::MissingArgument(_)
    ));
    assert!(matches!(
        parse("1.2.3").unwrap_err().kind,
        ParseErrorKind::BadNumber
    ));
}

#[test]
fn error_spans_are_byte_offsets_after_multibyte_text() {
    let err = parse("x+あ").unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::UnexpectedChar('あ'));
    assert_eq!(err.span, 2..5);
}

#[test]
fn reserved_variables_are_rejected() {
    assert!(matches!(
        parse("y=y+1").unwrap_err().kind,
        ParseErrorKind::ReservedVariable(_)
    ));
    assert!(matches!(
        parse("r=x").unwrap_err().kind,
        ParseErrorKind::ReservedVariable(_)
    ));
    assert!(matches!(
        parse("(x,t)").unwrap_err().kind,
        ParseErrorKind::ReservedVariable(_)
    ));
}

#[test]
fn unsupported_shapes_are_rejected() {
    assert!(matches!(
        parse("x=2").unwrap_err().kind,
        ParseErrorKind::UnsupportedLhs(_)
    ));
    assert!(matches!(
        parse("(1,2,3)").unwrap_err().kind,
        ParseErrorKind::BadTuple
    ));
}

fn deriv_close(src: &str, x: f64, expected: f64) {
    let d: Expr = parse_expression(src).unwrap().derivative("x");
    close(d.eval(&env("x", x, &[("a", 3.0)])), expected);
}

#[test]
fn symbolic_derivatives() {
    deriv_close("x^3", 2.0, 12.0);
    deriv_close("a x", 2.0, 3.0);
    deriv_close("\\sin x", 1.0, 1f64.cos());
    deriv_close("\\cos x", 1.0, -1f64.sin());
    deriv_close("\\tan x", 1.0, 1.0 / 1f64.cos().powi(2));
    deriv_close("e^{2x}", 1.0, 2.0 * (2f64).exp());
    deriv_close("\\ln x", 2.0, 0.5);
    deriv_close("\\log x", 2.0, 1.0 / (2.0 * 10f64.ln()));
    deriv_close("\\frac{1}{x}", 2.0, -0.25);
    deriv_close("x^{x}", 2.0, 4.0 * (2f64.ln() + 1.0));
    deriv_close("\\sqrt{x}", 4.0, 0.25);
    deriv_close("|x|", -2.0, -1.0);
    deriv_close("\\arctan x", 1.0, 0.5);
    deriv_close("\\sin(x^2)", 1.0, 2.0 * 1f64.cos());
    deriv_close("\\max(x,0)", 2.0, 1.0);
    deriv_close("\\lfloor x\\rfloor", 1.5, 0.0);
}
