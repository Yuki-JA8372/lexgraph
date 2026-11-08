//! Turns a [`Document`] into something drawable: curves, fills and analysis markers.
use super::model::{Display, Document, ExprEntry};
use crate::core::analysis::{find_extrema, find_intersections, find_roots, integrate};
use crate::core::{Env, Expr, Formula, FormulaKind, ParseError, parse, parse_expression};
use crate::render::{
    CurveSpec, LineStyle, Marker, MarkerKind, PlotSpec, SceneTheme, Viewport, auto_fit_y,
    sample_formula,
};
use std::cell::RefCell;
use std::collections::BTreeMap;

const ANALYSIS_SAMPLES: usize = 800;
const FIT_SAMPLES: usize = 400;
const FILL_SAMPLES: usize = 400;
const MAX_MARKERS: usize = 200;
const MAX_MARKER_LABELS: usize = 40;

struct CompiledEntry {
    id: u64,
    formula: Result<Formula, ParseError>,
}

/// The parse result of each entry; rebuilt only when a source changes.
pub struct Compiled {
    entries: Vec<CompiledEntry>,
}

impl Compiled {
    pub fn error_of(&self, id: u64) -> Option<&ParseError> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.formula.as_ref().err())
    }

    pub fn formula_of(&self, id: u64) -> Option<&Formula> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.formula.as_ref().ok())
    }
}

pub fn compile(doc: &Document) -> Compiled {
    Compiled {
        entries: doc
            .entries
            .iter()
            .map(|e| CompiledEntry {
                id: e.id,
                formula: parse(&e.source),
            })
            .collect(),
    }
}

/// Evaluates text with no variables, such as `2\pi`. Finite results only.
pub fn eval_constant(source: &str) -> Option<f64> {
    let value = parse_expression(source)
        .ok()?
        .eval(&Env::new("", BTreeMap::new()));
    value.is_finite().then_some(value)
}

/// The range to show when fitting automatically. `y=f(x)` curves keep the current
/// x range and set y, ignoring spikes; closed curves are always fully contained.
pub fn fit_view(doc: &Document, compiled: &Compiled) -> Viewport {
    let params = doc.param_values();
    let vp = doc.viewport;
    let mut function_lines = Vec::new();
    let mut curve_points: Vec<(f64, f64)> = Vec::new();
    for entry in doc.entries.iter().filter(|e| e.visible) {
        let Some(formula) = compiled.formula_of(entry.id) else {
            continue;
        };
        let lines = sample_formula(
            formula,
            &params,
            &vp,
            (entry.t_min, entry.t_max),
            FIT_SAMPLES,
        );
        if matches!(formula.kind, FormulaKind::Cartesian(_)) {
            function_lines.extend(lines);
        } else {
            curve_points.extend(lines.into_iter().flatten());
        }
    }

    let function_y = auto_fit_y(&function_lines);
    if function_y.is_none() && curve_points.is_empty() {
        return vp;
    }
    let (mut y_min, mut y_max) = function_y.unwrap_or((f64::INFINITY, f64::NEG_INFINITY));
    let (mut x_min, mut x_max) = (vp.x_min, vp.x_max);
    if !curve_points.is_empty() {
        let extent = |pick: fn(&(f64, f64)) -> f64| {
            curve_points
                .iter()
                .map(pick)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v), hi.max(v))
                })
        };
        let (cx_min, cx_max) = extent(|p| p.0);
        let (cy_min, cy_max) = extent(|p| p.1);
        let pad_x = (cx_max - cx_min) * 0.05;
        let pad_y = (cy_max - cy_min) * 0.05;
        x_min = x_min.min(cx_min - pad_x);
        x_max = x_max.max(cx_max + pad_x);
        y_min = y_min.min(cy_min - pad_y);
        y_max = y_max.max(cy_max + pad_y);
    }
    let fitted = Viewport {
        x_min,
        x_max,
        y_min,
        y_max,
    };
    if fitted.is_valid() { fitted } else { vp }
}

pub struct PlotResult {
    pub spec: PlotSpec,
    /// Definite integral per entry using a fill, or the reason it failed.
    pub integrals: BTreeMap<u64, Result<f64, String>>,
}

/// A compact number: up to four decimals, no trailing zeros, no `-0`.
pub fn short_number(v: f64) -> String {
    if v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-3) {
        if v.abs() < 1e-12 {
            return "0".into();
        }
        return format!("{v:.3e}");
    }
    let text = format!("{v:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" || text.is_empty() {
        "0".into()
    } else {
        text.to_string()
    }
}

/// Like [`short_number`], but a value tiny compared to the visible span is numeric noise.
fn view_number(v: f64, span: f64) -> String {
    if v.abs() < span * 1e-6 {
        "0".into()
    } else {
        short_number(v)
    }
}

fn evaluator<'a>(
    expr: &'a Expr,
    var: &str,
    params: &BTreeMap<String, f64>,
) -> impl Fn(f64) -> f64 + 'a {
    let env = RefCell::new(Env::new(var, params.clone()));
    move |x| {
        let mut env = env.borrow_mut();
        env.var = x;
        expr.eval(&env)
    }
}

/// The curve of an entry's derivative: `f'(x)`, `dy/dx` against `x`, or `r'(θ)`.
fn derivative_formula(formula: &Formula) -> Formula {
    let var = formula.kind.main_var();
    let kind = match &formula.kind {
        FormulaKind::Cartesian(e) => FormulaKind::Cartesian(e.derivative(var)),
        FormulaKind::Parametric { x, y } => FormulaKind::Parametric {
            x: x.clone(),
            y: Expr::Div(Box::new(y.derivative(var)), Box::new(x.derivative(var))),
        },
        FormulaKind::Polar(e) => FormulaKind::Polar(e.derivative(var)),
    };
    Formula {
        kind,
        params: formula.params.clone(),
    }
}

fn eval_bound(source: &str, params: &BTreeMap<String, f64>) -> Result<f64, String> {
    let expr = parse_expression(source).map_err(|e| format!("積分範囲「{source}」: {e}"))?;
    let value = expr.eval(&Env::new("", params.clone()));
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("積分範囲「{source}」を計算できません"))
    }
}

struct Fill {
    polygon: Vec<(f64, f64)>,
    value: f64,
}

fn integral_fill(
    entry: &ExprEntry,
    formula: &Formula,
    params: &BTreeMap<String, f64>,
) -> Result<Fill, String> {
    let FormulaKind::Cartesian(expr) = &formula.kind else {
        return Err("定積分の塗りは y=f(x) の式で使えます".into());
    };
    let a = eval_bound(&entry.integral.a, params)?;
    let b = eval_bound(&entry.integral.b, params)?;
    let f = evaluator(expr, formula.kind.main_var(), params);
    let value = integrate(&f, a, b);
    if value.is_nan() {
        return Err("この範囲では関数が定義されていない点があります".into());
    }
    let mut polygon = vec![(a, 0.0)];
    for i in 0..=FILL_SAMPLES {
        let x = a + (b - a) * i as f64 / FILL_SAMPLES as f64;
        let y = f(x);
        if y.is_finite() {
            polygon.push((x, y));
        }
    }
    polygon.push((b, 0.0));
    Ok(Fill { polygon, value })
}

/// Builds the full plot description for the document's current view.
pub fn build_plot(
    doc: &Document,
    compiled: &Compiled,
    theme: SceneTheme,
    pixels: usize,
) -> PlotResult {
    let params = doc.param_values();
    let vp = doc.viewport;
    let mut curves = Vec::new();
    let mut integrals = BTreeMap::new();
    let mut analyzable: Vec<(&ExprEntry, &Expr)> = Vec::new();

    for entry in doc.entries.iter().filter(|e| e.visible) {
        let Some(formula) = compiled.formula_of(entry.id) else {
            continue;
        };
        let range = (entry.t_min, entry.t_max);
        let mut label = entry.source.trim().to_string();
        let mut fill = None;
        if entry.integral.enabled {
            match integral_fill(entry, formula, &params) {
                Ok(done) => {
                    label = format!("{label}  ∫={}", short_number(done.value));
                    integrals.insert(entry.id, Ok(done.value));
                    fill = Some(done.polygon);
                }
                Err(reason) => {
                    integrals.insert(entry.id, Err(reason));
                }
            }
        }
        curves.push(CurveSpec {
            label: label.clone(),
            color: entry.color,
            width: entry.width,
            style: entry.style,
            lines: sample_formula(formula, &params, &vp, range, pixels),
            fill,
        });
        if entry.show_derivative {
            let derivative = derivative_formula(formula);
            curves.push(CurveSpec {
                label: format!("({})'", entry.source.trim()),
                color: entry.color,
                width: (entry.width * 0.8).max(1.0),
                style: LineStyle::Dashed,
                lines: sample_formula(&derivative, &params, &vp, range, pixels),
                fill: None,
            });
        }
        if let FormulaKind::Cartesian(expr) = &formula.kind {
            analyzable.push((entry, expr));
        }
    }

    let markers = if doc.display.show_points {
        analysis_markers(&analyzable, &params, doc)
    } else {
        Vec::new()
    };

    PlotResult {
        spec: plot_spec(doc.display, doc, curves, markers, theme),
        integrals,
    }
}

fn plot_spec(
    display: Display,
    doc: &Document,
    curves: Vec<CurveSpec>,
    markers: Vec<Marker>,
    theme: SceneTheme,
) -> PlotSpec {
    PlotSpec {
        viewport: doc.viewport,
        curves,
        markers,
        show_grid: display.show_grid,
        show_ticks: display.show_ticks,
        show_axis_labels: display.show_axis_labels,
        show_legend: display.show_legend,
        theme,
    }
}

/// Finds the roots, extrema and intersections of the `y=f(x)` curves in view.
fn analysis_markers(
    curves: &[(&ExprEntry, &Expr)],
    params: &BTreeMap<String, f64>,
    doc: &Document,
) -> Vec<Marker> {
    let vp = doc.viewport;
    let (a, b) = (vp.x_min, vp.x_max);
    let n = ANALYSIS_SAMPLES;
    let in_view = |y: f64| y >= vp.y_min && y <= vp.y_max;
    let mut markers = Vec::new();

    let functions: Vec<_> = curves
        .iter()
        .map(|(entry, expr)| (entry.color, evaluator(expr, "x", params)))
        .collect();

    for (i, (color, f)) in functions.iter().enumerate() {
        for x in find_roots(f, a, b, n) {
            markers.push(new_marker(x, 0.0, MarkerKind::Root, *color));
        }
        for e in find_extrema(f, a, b, n)
            .into_iter()
            .filter(|e| in_view(e.y))
        {
            markers.push(new_marker(e.x, e.y, MarkerKind::Extremum, *color));
        }
        for (_, g) in &functions[i + 1..] {
            for (x, y) in find_intersections(f, g, a, b, n) {
                if in_view(y) {
                    markers.push(new_marker(x, y, MarkerKind::Intersection, *color));
                }
            }
        }
    }

    markers.truncate(MAX_MARKERS);
    for marker in markers.iter_mut().take(MAX_MARKER_LABELS) {
        marker.label = Some(format!(
            "({}, {})",
            view_number(marker.x, vp.width()),
            view_number(marker.y, vp.height())
        ));
    }
    markers
}

fn new_marker(x: f64, y: f64, kind: MarkerKind, color: crate::render::Rgb) -> Marker {
    Marker {
        x,
        y,
        kind,
        color,
        label: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapped {
    pub x: f64,
    pub y: f64,
    /// Index into `spec.curves`.
    pub curve: usize,
}

/// Finds the curve point closest to the cursor, if one is within `reach` pixels.
pub fn snap_to_curve(
    spec: &PlotSpec,
    cursor: (f32, f32),
    w: f32,
    h: f32,
    reach: f32,
) -> Option<Snapped> {
    let vp = spec.viewport;
    let mut best: Option<(f32, Snapped)> = None;
    for (index, curve) in spec.curves.iter().enumerate() {
        for line in &curve.lines {
            for pair in line.windows(2) {
                let p = vp.to_px(pair[0].0, pair[0].1, w, h);
                let q = vp.to_px(pair[1].0, pair[1].1, w, h);
                let (d, u) = distance_to_segment(cursor, p, q);
                if d <= reach && best.is_none_or(|(b, _)| d < b) {
                    let u = f64::from(u);
                    best = Some((
                        d,
                        Snapped {
                            x: pair[0].0 + (pair[1].0 - pair[0].0) * u,
                            y: pair[0].1 + (pair[1].1 - pair[0].1) * u,
                            curve: index,
                        },
                    ));
                }
            }
        }
    }
    best.map(|b| b.1)
}

/// Distance from `c` to segment `pq`, and the position along it from 0 to 1.
fn distance_to_segment(c: (f32, f32), p: (f32, f32), q: (f32, f32)) -> (f32, f32) {
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    let len2 = dx * dx + dy * dy;
    let u = if len2 > 0.0 {
        (((c.0 - p.0) * dx + (c.1 - p.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (nx, ny) = (p.0 + dx * u, p.1 + dy * u);
    ((c.0 - nx).hypot(c.1 - ny), u)
}
