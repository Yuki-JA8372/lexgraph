//! Numeric analysis of `y = f(x)` curves: roots, extrema, intersections, integrals.

type F<'a> = &'a dyn Fn(f64) -> f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtremumKind {
    Max,
    Min,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extremum {
    pub x: f64,
    pub y: f64,
    pub kind: ExtremumKind,
}

fn samples(f: F, a: f64, b: f64, n: usize) -> Vec<(f64, f64)> {
    (0..=n)
        .map(|i| {
            let x = a + (b - a) * i as f64 / n as f64;
            (x, f(x))
        })
        .collect()
}

/// Narrows a sign change in `[lo, hi]` down to a point.
fn bisect(f: F, mut lo: f64, mut hi: f64) -> f64 {
    let mut f_lo = f(lo);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        let f_mid = f(mid);
        if f_mid == 0.0 {
            return mid;
        }
        if (f_lo < 0.0) == (f_mid < 0.0) {
            lo = mid;
            f_lo = f_mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Tells a real change of value from a jump or pole: a continuous function's
/// gap shrinks as the interval narrows, while a jump's does not.
fn is_continuous(f: F, mut a: f64, mut b: f64) -> bool {
    let (mut ya, mut yb) = (f(a), f(b));
    if !ya.is_finite() || !yb.is_finite() {
        return false;
    }
    let initial = (yb - ya).abs();
    if initial == 0.0 {
        return true;
    }
    for _ in 0..60 {
        let m = 0.5 * (a + b);
        let ym = f(m);
        if !ym.is_finite() {
            return false;
        }
        if (ym - ya).abs() >= (yb - ym).abs() {
            b = m;
            yb = ym;
        } else {
            a = m;
            ya = ym;
        }
        if (yb - ya).abs() < initial * 0.01 {
            return true;
        }
    }
    false
}

/// Finds where `f` crosses or touches zero, excluding poles and jumps.
pub fn find_roots(f: F, a: f64, b: f64, n: usize) -> Vec<f64> {
    let points = samples(f, a, b, n);
    let mut roots = Vec::new();
    for (i, pair) in points.windows(2).enumerate() {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if y0 == 0.0 {
            // A run of zeros means the curve lies on the axis: no isolated root.
            let prev_zero = i > 0 && points[i - 1].1 == 0.0;
            if !prev_zero && y1 != 0.0 {
                roots.push(x0);
            }
        } else if y0.is_finite() && y1.is_finite() && y1 != 0.0 && (y0 < 0.0) != (y1 < 0.0) {
            let root = bisect(f, x0, x1);
            let tolerance = 1e-6 * (1.0 + y0.abs().max(y1.abs()));
            if f(root).abs() <= tolerance {
                roots.push(root);
            }
        }
    }
    let flat_tolerance = |y: f64| y.abs() < 1e-7;
    roots.extend(
        find_extrema(f, a, b, n)
            .iter()
            .filter(|e| flat_tolerance(e.y))
            .map(|e| e.x),
    );
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|p, q| (*p - *q).abs() < (b - a) / n as f64);
    roots
}

/// Golden-section search for the maximum of `sign * f` on `[a, b]`.
fn golden_max(f: F, sign: f64, mut a: f64, mut b: f64) -> f64 {
    const RATIO: f64 = 0.618_033_988_749_894_8;
    let score = |x: f64| {
        let y = sign * f(x);
        if y.is_nan() { f64::NEG_INFINITY } else { y }
    };
    let mut c = b - RATIO * (b - a);
    let mut d = a + RATIO * (b - a);
    let (mut fc, mut fd) = (score(c), score(d));
    for _ in 0..70 {
        if fc > fd {
            b = d;
            d = c;
            fd = fc;
            c = b - RATIO * (b - a);
            fc = score(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + RATIO * (b - a);
            fd = score(d);
        }
    }
    0.5 * (a + b)
}

/// Finds local maxima and minima, skipping flat runs, jumps and poles.
pub fn find_extrema(f: F, a: f64, b: f64, n: usize) -> Vec<Extremum> {
    let points = samples(f, a, b, n);
    let mut found = Vec::new();
    for w in points.windows(3) {
        let ((x0, y0), (x1, y1), (x2, y2)) = (w[0], w[1], w[2]);
        if !(y0.is_finite() && y1.is_finite() && y2.is_finite()) {
            continue;
        }
        let kind = if y1 > y0 && y1 > y2 {
            ExtremumKind::Max
        } else if y1 < y0 && y1 < y2 {
            ExtremumKind::Min
        } else {
            continue;
        };
        if !is_continuous(f, x0, x1) || !is_continuous(f, x1, x2) {
            continue;
        }
        let sign = if kind == ExtremumKind::Max { 1.0 } else { -1.0 };
        let x = golden_max(f, sign, x0, x2);
        found.push(Extremum { x, y: f(x), kind });
    }
    found
}

/// Finds points where two curves meet, as `(x, y)`.
pub fn find_intersections(f: F, g: F, a: f64, b: f64, n: usize) -> Vec<(f64, f64)> {
    let difference = |x: f64| f(x) - g(x);
    find_roots(&difference, a, b, n)
        .into_iter()
        .map(|x| (x, f(x)))
        .collect()
}

/// Signed definite integral by composite Simpson. NaN if `f` is undefined anywhere sampled.
pub fn integrate(f: F, a: f64, b: f64) -> f64 {
    const N: usize = 4000;
    let h = (b - a) / N as f64;
    let mut sum = f(a) + f(b);
    for i in 1..N {
        let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
        sum += weight * f(a + h * i as f64);
    }
    if sum.is_finite() {
        sum * h / 3.0
    } else {
        f64::NAN
    }
}
