use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::LN_10;

/// One-argument functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Cot,
    Sec,
    Csc,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Ln,
    Exp,
    Sqrt,
    Abs,
    Floor,
    Ceil,
    Sign,
}

impl Func {
    fn apply(self, v: f64) -> f64 {
        match self {
            Func::Sin => v.sin(),
            Func::Cos => v.cos(),
            Func::Tan => v.tan(),
            Func::Cot => 1.0 / v.tan(),
            Func::Sec => 1.0 / v.cos(),
            Func::Csc => 1.0 / v.sin(),
            Func::Asin => v.asin(),
            Func::Acos => v.acos(),
            Func::Atan => v.atan(),
            Func::Sinh => v.sinh(),
            Func::Cosh => v.cosh(),
            Func::Tanh => v.tanh(),
            Func::Ln => v.ln(),
            Func::Exp => v.exp(),
            Func::Sqrt => v.sqrt(),
            Func::Abs => v.abs(),
            Func::Floor => v.floor(),
            Func::Ceil => v.ceil(),
            Func::Sign => {
                if v.is_nan() {
                    f64::NAN
                } else if v == 0.0 {
                    0.0
                } else {
                    v.signum()
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    /// n-th root; unlike `x^(1/n)` it is real for negative x when n is odd.
    Root(Box<Expr>, Box<Expr>),
    /// `Log(base, value)`.
    Log(Box<Expr>, Box<Expr>),
    Call(Func, Box<Expr>),
    Min(Box<Expr>, Box<Expr>),
    Max(Box<Expr>, Box<Expr>),
}

/// Values available while evaluating: the main variable plus parameters.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub var_name: String,
    pub var: f64,
    pub params: BTreeMap<String, f64>,
}

impl Env {
    pub fn new(var_name: &str, params: BTreeMap<String, f64>) -> Self {
        Self {
            var_name: var_name.to_string(),
            var: 0.0,
            params,
        }
    }
}

fn num(v: f64) -> Expr {
    Expr::Num(v)
}

fn neg(a: Expr) -> Expr {
    match a {
        Expr::Num(v) => num(-v),
        Expr::Neg(inner) => *inner,
        other => Expr::Neg(Box::new(other)),
    }
}

fn add(a: Expr, b: Expr) -> Expr {
    match (a, b) {
        (Expr::Num(x), Expr::Num(y)) => num(x + y),
        (Expr::Num(0.0), other) | (other, Expr::Num(0.0)) => other,
        (a, b) => Expr::Add(Box::new(a), Box::new(b)),
    }
}

fn sub(a: Expr, b: Expr) -> Expr {
    match (a, b) {
        (Expr::Num(x), Expr::Num(y)) => num(x - y),
        (a, Expr::Num(0.0)) => a,
        (Expr::Num(0.0), b) => neg(b),
        (a, b) => Expr::Sub(Box::new(a), Box::new(b)),
    }
}

fn mul(a: Expr, b: Expr) -> Expr {
    match (a, b) {
        (Expr::Num(x), Expr::Num(y)) => num(x * y),
        (Expr::Num(0.0), _) | (_, Expr::Num(0.0)) => num(0.0),
        (Expr::Num(1.0), other) | (other, Expr::Num(1.0)) => other,
        (a, b) => Expr::Mul(Box::new(a), Box::new(b)),
    }
}

fn div(a: Expr, b: Expr) -> Expr {
    match (a, b) {
        (Expr::Num(0.0), _) => num(0.0),
        (a, Expr::Num(1.0)) => a,
        (a, b) => Expr::Div(Box::new(a), Box::new(b)),
    }
}

fn pow(a: Expr, b: Expr) -> Expr {
    match (a, b) {
        (_, Expr::Num(0.0)) => num(1.0),
        (a, Expr::Num(1.0)) => a,
        (a, b) => Expr::Pow(Box::new(a), Box::new(b)),
    }
}

fn call(f: Func, a: Expr) -> Expr {
    Expr::Call(f, Box::new(a))
}

impl Expr {
    /// Evaluates the expression. A parameter with no value makes the result NaN.
    pub fn eval(&self, env: &Env) -> f64 {
        match self {
            Expr::Num(v) => *v,
            Expr::Var(name) => {
                if *name == env.var_name {
                    env.var
                } else {
                    env.params.get(name).copied().unwrap_or(f64::NAN)
                }
            }
            Expr::Neg(a) => -a.eval(env),
            Expr::Add(a, b) => a.eval(env) + b.eval(env),
            Expr::Sub(a, b) => a.eval(env) - b.eval(env),
            Expr::Mul(a, b) => a.eval(env) * b.eval(env),
            Expr::Div(a, b) => a.eval(env) / b.eval(env),
            Expr::Pow(a, b) => a.eval(env).powf(b.eval(env)),
            Expr::Root(n, a) => {
                let (n, v) = (n.eval(env), a.eval(env));
                if v < 0.0 && n.fract() == 0.0 && (n as i64) % 2 != 0 {
                    -(-v).powf(1.0 / n)
                } else {
                    v.powf(1.0 / n)
                }
            }
            Expr::Log(base, a) => a.eval(env).ln() / base.eval(env).ln(),
            Expr::Call(f, a) => f.apply(a.eval(env)),
            Expr::Min(a, b) => nan_aware(a.eval(env), b.eval(env), f64::min),
            Expr::Max(a, b) => nan_aware(a.eval(env), b.eval(env), f64::max),
        }
    }

    /// True if `var` appears anywhere in the expression.
    pub fn depends_on(&self, var: &str) -> bool {
        match self {
            Expr::Num(_) => false,
            Expr::Var(name) => name == var,
            Expr::Neg(a) | Expr::Call(_, a) => a.depends_on(var),
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Pow(a, b)
            | Expr::Root(a, b)
            | Expr::Log(a, b)
            | Expr::Min(a, b)
            | Expr::Max(a, b) => a.depends_on(var) || b.depends_on(var),
        }
    }

    /// Adds every variable name used by the expression to `out`.
    pub fn collect_vars(&self, out: &mut BTreeSet<String>) {
        match self {
            Expr::Num(_) => {}
            Expr::Var(name) => {
                out.insert(name.clone());
            }
            Expr::Neg(a) | Expr::Call(_, a) => a.collect_vars(out),
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Pow(a, b)
            | Expr::Root(a, b)
            | Expr::Log(a, b)
            | Expr::Min(a, b)
            | Expr::Max(a, b) => {
                a.collect_vars(out);
                b.collect_vars(out);
            }
        }
    }

    /// Symbolic derivative with respect to `var`.
    pub fn derivative(&self, var: &str) -> Expr {
        let d = |e: &Expr| e.derivative(var);
        match self {
            Expr::Num(_) => num(0.0),
            Expr::Var(name) => num(if name == var { 1.0 } else { 0.0 }),
            Expr::Neg(a) => neg(d(a)),
            Expr::Add(a, b) => add(d(a), d(b)),
            Expr::Sub(a, b) => sub(d(a), d(b)),
            Expr::Mul(a, b) => add(mul(d(a), (**b).clone()), mul((**a).clone(), d(b))),
            Expr::Div(a, b) => div(
                sub(mul(d(a), (**b).clone()), mul((**a).clone(), d(b))),
                pow((**b).clone(), num(2.0)),
            ),
            Expr::Pow(a, b) => self.derive_pow(a, b, var),
            Expr::Root(n, a) => {
                // d/dx root(n, u) = root(n, u) * u' / (n * u)
                div(mul(self.clone(), d(a)), mul((**n).clone(), (**a).clone()))
            }
            Expr::Log(base, a) => {
                let quotient = Expr::Div(
                    Box::new(call(Func::Ln, (**a).clone())),
                    Box::new(call(Func::Ln, (**base).clone())),
                );
                match **base {
                    // The common case avoids carrying ln(10) symbolically.
                    Expr::Num(10.0) => div(d(a), mul((**a).clone(), num(LN_10))),
                    _ => quotient.derivative(var),
                }
            }
            Expr::Call(f, a) => derive_call(*f, a, d(a)),
            // min(a,b) = (a+b-|a-b|)/2 and max(a,b) = (a+b+|a-b|)/2
            Expr::Min(a, b) => {
                let half_sum = add((**a).clone(), (**b).clone());
                let gap = call(Func::Abs, sub((**a).clone(), (**b).clone()));
                div(sub(half_sum, gap), num(2.0)).derivative(var)
            }
            Expr::Max(a, b) => {
                let half_sum = add((**a).clone(), (**b).clone());
                let gap = call(Func::Abs, sub((**a).clone(), (**b).clone()));
                div(add(half_sum, gap), num(2.0)).derivative(var)
            }
        }
    }

    fn derive_pow(&self, a: &Expr, b: &Expr, var: &str) -> Expr {
        let da = a.derivative(var);
        let db = b.derivative(var);
        if !b.depends_on(var) {
            // Constant exponent: n * u^(n-1) * u'. Stays defined for negative u.
            let lowered = pow(a.clone(), sub(b.clone(), num(1.0)));
            return mul(mul(b.clone(), lowered), da);
        }
        if !a.depends_on(var) {
            return mul(mul(self.clone(), call(Func::Ln, a.clone())), db);
        }
        let log_part = mul(db, call(Func::Ln, a.clone()));
        let ratio_part = div(mul(b.clone(), da), a.clone());
        mul(self.clone(), add(log_part, ratio_part))
    }
}

fn nan_aware(a: f64, b: f64, f: fn(f64, f64) -> f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        f(a, b)
    }
}

fn derive_call(f: Func, a: &Expr, da: Expr) -> Expr {
    let u = || a.clone();
    let squared = |e: Expr| pow(e, num(2.0));
    let one_minus_u2 = || sub(num(1.0), squared(u()));
    match f {
        Func::Sin => mul(call(Func::Cos, u()), da),
        Func::Cos => neg(mul(call(Func::Sin, u()), da)),
        Func::Tan => div(da, squared(call(Func::Cos, u()))),
        Func::Cot => neg(div(da, squared(call(Func::Sin, u())))),
        Func::Sec => mul(mul(call(Func::Sec, u()), call(Func::Tan, u())), da),
        Func::Csc => neg(mul(mul(call(Func::Csc, u()), call(Func::Cot, u())), da)),
        Func::Asin => div(da, call(Func::Sqrt, one_minus_u2())),
        Func::Acos => neg(div(da, call(Func::Sqrt, one_minus_u2()))),
        Func::Atan => div(da, add(num(1.0), squared(u()))),
        Func::Sinh => mul(call(Func::Cosh, u()), da),
        Func::Cosh => mul(call(Func::Sinh, u()), da),
        Func::Tanh => div(da, squared(call(Func::Cosh, u()))),
        Func::Ln => div(da, u()),
        Func::Exp => mul(call(Func::Exp, u()), da),
        Func::Sqrt => div(da, mul(num(2.0), call(Func::Sqrt, u()))),
        Func::Abs => mul(call(Func::Sign, u()), da),
        Func::Floor | Func::Ceil | Func::Sign => num(0.0),
    }
}
