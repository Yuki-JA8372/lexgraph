use super::viewport::Viewport;
use crate::core::{Env, Formula, FormulaKind};
use std::collections::BTreeMap;

type Point = (f64, f64);

const CURVE_SAMPLES: usize = 2000;
const MAX_X_SAMPLES: usize = 4000;
/// Steps longer than this fraction of the view get checked for a discontinuity.
const JUMP_CHECK: f64 = 0.02;
const REFINE_STEPS: usize = 48;

/// Samples a formula over the viewport (`y=f(x)`) or `range` (parametric, polar).
/// Each returned polyline is an unbroken piece of the curve.
pub fn sample_formula(
    formula: &Formula,
    params: &BTreeMap<String, f64>,
    vp: &Viewport,
    range: (f64, f64),
    pixels: usize,
) -> Vec<Vec<Point>> {
    let mut env = Env::new(formula.kind.main_var(), params.clone());
    match &formula.kind {
        FormulaKind::Cartesian(e) => {
            let n = (pixels.max(2) * 2).min(MAX_X_SAMPLES);
            trace(
                &mut |x| {
                    env.var = x;
                    (x, e.eval(&env))
                },
                (vp.x_min, vp.x_max),
                n,
                vp,
            )
        }
        FormulaKind::Parametric { x, y } => trace(
            &mut |t| {
                env.var = t;
                (x.eval(&env), y.eval(&env))
            },
            range,
            CURVE_SAMPLES,
            vp,
        ),
        FormulaKind::Polar(e) => trace(
            &mut |t| {
                env.var = t;
                let r = e.eval(&env);
                (r * t.cos(), r * t.sin())
            },
            range,
            CURVE_SAMPLES,
            vp,
        ),
    }
}

fn finite(p: Point) -> bool {
    p.0.is_finite() && p.1.is_finite()
}

fn trace(
    f: &mut dyn FnMut(f64) -> Point,
    (a, b): (f64, f64),
    n: usize,
    vp: &Viewport,
) -> Vec<Vec<Point>> {
    let gap = |p: Point, q: Point| {
        let dx = (p.0 - q.0) / vp.width();
        let dy = (p.1 - q.1) / vp.height();
        dx.hypot(dy)
    };
    let mut lines = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    let mut previous: Option<(f64, Point)> = None;

    for i in 0..=n {
        let t = a + (b - a) * i as f64 / n as f64;
        let p = f(t);
        if let Some((t0, p0)) = previous {
            match (finite(p0), finite(p)) {
                (true, true) if gap(p0, p) > JUMP_CHECK => {
                    if let Some((left, right)) = find_break(f, t0, t, &gap) {
                        current.push(left);
                        finish(&mut lines, &mut current);
                        current.push(right);
                    }
                }
                (true, false) => {
                    current.push(domain_edge(f, t0, t));
                    finish(&mut lines, &mut current);
                }
                (false, true) => current.push(domain_edge(f, t, t0)),
                _ => {}
            }
        }
        if finite(p) {
            current.push(p);
        }
        previous = Some((t, p));
    }
    finish(&mut lines, &mut current);
    lines
}

fn finish(lines: &mut Vec<Vec<Point>>, current: &mut Vec<Point>) {
    let line = std::mem::take(current);
    if line.len() >= 2 {
        lines.push(line);
    }
}

/// Decides if a long step is a real discontinuity. Narrowing the interval around
/// the larger gap shrinks that gap for a steep curve but not for a jump or pole.
/// Returns the points on each side of the break.
fn find_break(
    f: &mut dyn FnMut(f64) -> Point,
    mut t0: f64,
    mut t1: f64,
    gap: &dyn Fn(Point, Point) -> f64,
) -> Option<(Point, Point)> {
    let (mut p0, mut p1) = (f(t0), f(t1));
    let initial = gap(p0, p1);
    for _ in 0..REFINE_STEPS {
        let tm = 0.5 * (t0 + t1);
        let pm = f(tm);
        if !finite(pm) {
            let left = domain_edge(f, t0, tm);
            let right = domain_edge(f, t1, tm);
            return Some((left, right));
        }
        if gap(p0, pm) >= gap(pm, p1) {
            t1 = tm;
            p1 = pm;
        } else {
            t0 = tm;
            p0 = pm;
        }
        if gap(p0, p1) < initial * 0.1 {
            return None;
        }
    }
    Some((p0, p1))
}

/// Walks from a defined parameter towards an undefined one to find the last defined point.
fn domain_edge(f: &mut dyn FnMut(f64) -> Point, mut inside: f64, mut outside: f64) -> Point {
    let mut best = f(inside);
    for _ in 0..REFINE_STEPS {
        let mid = 0.5 * (inside + outside);
        let p = f(mid);
        if finite(p) {
            inside = mid;
            best = p;
        } else {
            outside = mid;
        }
    }
    best
}

/// Picks a y range for the curves, ignoring the extreme values near poles.
pub fn auto_fit_y(lines: &[Vec<Point>]) -> Option<(f64, f64)> {
    let mut ys: Vec<f64> = lines
        .iter()
        .flatten()
        .map(|p| p.1)
        .filter(|y| y.is_finite())
        .collect();
    if ys.is_empty() {
        return None;
    }
    ys.sort_by(f64::total_cmp);
    let at = |q: f64| ys[((ys.len() - 1) as f64 * q) as usize];
    let (lo, hi) = (at(0.03), at(0.97));
    let span = hi - lo;
    if span < 1e-9 {
        return Some((lo - 1.0, hi + 1.0));
    }
    Some((lo - span * 0.1, hi + span * 0.1))
}
