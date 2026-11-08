use super::error::{ParseError, ParseErrorKind as Kind};
use super::expr::{Expr, Func};
use std::collections::BTreeSet;
use std::f64::consts::{E, PI};
use std::ops::Range;

/// What a formula draws, and over which main variable.
#[derive(Debug, Clone, PartialEq)]
pub enum FormulaKind {
    /// `y = f(x)`
    Cartesian(Expr),
    /// `(x(t), y(t))`
    Parametric { x: Expr, y: Expr },
    /// `r = f(\theta)`
    Polar(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    pub kind: FormulaKind,
    /// Free names other than the main variable, sorted.
    pub params: Vec<String>,
}

pub const VAR_X: &str = "x";
pub const VAR_T: &str = "t";
pub const VAR_THETA: &str = "\\theta";

impl FormulaKind {
    pub fn main_var(&self) -> &'static str {
        match self {
            FormulaKind::Cartesian(_) => VAR_X,
            FormulaKind::Parametric { .. } => VAR_T,
            FormulaKind::Polar(_) => VAR_THETA,
        }
    }
}

type Res<T> = Result<T, ParseError>;

fn err<T>(kind: Kind, span: Range<usize>) -> Res<T> {
    Err(ParseError { kind, span })
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(String),
    Ident(char),
    Cmd(String),
    Sym(char),
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    span: Range<usize>,
}

const SPACING_COMMANDS: [&str; 8] = [
    "\\left", "\\right", "\\,", "\\;", "\\:", "\\!", "\\quad", "\\qquad",
];

fn lex(src: &str) -> Res<Vec<Token>> {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let end_of = |i: usize| chars.get(i).map_or(src.len(), |c| c.0);
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '0'..='9' | '.' => {
                let mut j = i;
                while j < chars.len() && (chars[j].1.is_ascii_digit() || chars[j].1 == '.') {
                    j += 1;
                }
                let span = start..end_of(j);
                let text = &src[span.clone()];
                if text.parse::<f64>().is_err() {
                    return err(Kind::BadNumber, span);
                }
                out.push(Token {
                    tok: Tok::Num(text.to_string()),
                    span,
                });
                i = j;
            }
            'a'..='z' | 'A'..='Z' => {
                out.push(Token {
                    tok: Tok::Ident(c),
                    span: start..end_of(i + 1),
                });
                i += 1;
            }
            '\\' => {
                let mut j = i + 1;
                if chars.get(j).is_some_and(|c| c.1.is_ascii_alphabetic()) {
                    while chars.get(j).is_some_and(|c| c.1.is_ascii_alphabetic()) {
                        j += 1;
                    }
                } else if j < chars.len() {
                    j += 1;
                }
                let span = start..end_of(j);
                let name = src[span.clone()].to_string();
                if !SPACING_COMMANDS.contains(&name.as_str()) && name != "\\ " {
                    out.push(Token {
                        tok: Tok::Cmd(name),
                        span,
                    });
                }
                i = j;
            }
            '+' | '-' | '*' | '/' | '^' | '_' | '(' | ')' | '{' | '}' | '[' | ']' | '|' | ','
            | '=' => {
                out.push(Token {
                    tok: Tok::Sym(c),
                    span: start..end_of(i + 1),
                });
                i += 1;
            }
            other => return err(Kind::UnexpectedChar(other), start..end_of(i + 1)),
        }
    }
    Ok(out)
}

fn func_from_cmd(name: &str) -> Option<Func> {
    Some(match name {
        "\\sin" => Func::Sin,
        "\\cos" => Func::Cos,
        "\\tan" => Func::Tan,
        "\\cot" => Func::Cot,
        "\\sec" => Func::Sec,
        "\\csc" => Func::Csc,
        "\\arcsin" => Func::Asin,
        "\\arccos" => Func::Acos,
        "\\arctan" => Func::Atan,
        "\\sinh" => Func::Sinh,
        "\\cosh" => Func::Cosh,
        "\\tanh" => Func::Tanh,
        "\\ln" => Func::Ln,
        "\\exp" => Func::Exp,
        "\\sgn" => Func::Sign,
        _ => return None,
    })
}

fn inverse_of(f: Func) -> Option<Func> {
    Some(match f {
        Func::Sin => Func::Asin,
        Func::Cos => Func::Acos,
        Func::Tan => Func::Atan,
        _ => return None,
    })
}

fn is_greek(name: &str) -> bool {
    const GREEK: [&str; 37] = [
        "alpha",
        "beta",
        "gamma",
        "delta",
        "epsilon",
        "varepsilon",
        "zeta",
        "eta",
        "theta",
        "vartheta",
        "iota",
        "kappa",
        "lambda",
        "mu",
        "nu",
        "xi",
        "rho",
        "varrho",
        "sigma",
        "tau",
        "upsilon",
        "phi",
        "varphi",
        "chi",
        "psi",
        "omega",
        "Gamma",
        "Delta",
        "Theta",
        "Lambda",
        "Xi",
        "Pi",
        "Sigma",
        "Upsilon",
        "Phi",
        "Psi",
        "Omega",
    ];
    GREEK.contains(&name.trim_start_matches('\\'))
}

/// Commands that continue an expression as an operator or close a bracket.
fn is_non_value_cmd(name: &str) -> bool {
    matches!(
        name,
        "\\cdot" | "\\times" | "\\div" | "\\rfloor" | "\\rceil"
    )
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    end: usize,
    /// Nesting depth of open `|`, to decide if the next `|` closes one.
    abs_depth: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>, end: usize) -> Self {
        Self {
            tokens,
            pos: 0,
            end,
            abs_depth: 0,
        }
    }

    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos).map(|t| &t.tok)
    }

    fn peek_sym(&self, c: char) -> bool {
        self.peek() == Some(&Tok::Sym(c))
    }

    fn here(&self) -> Range<usize> {
        self.tokens
            .get(self.pos)
            .map_or(self.end..self.end, |t| t.span.clone())
    }

    fn eat_sym(&mut self, c: char) -> bool {
        if self.peek_sym(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_close(&mut self, c: char, open: &str, open_span: Range<usize>) -> Res<()> {
        if self.eat_sym(c) {
            Ok(())
        } else {
            err(Kind::Unclosed(open.to_string()), open_span)
        }
    }

    fn starts_value(&self) -> bool {
        match self.peek() {
            Some(Tok::Num(_) | Tok::Ident(_)) => true,
            Some(Tok::Cmd(name)) => !is_non_value_cmd(name),
            Some(Tok::Sym('(' | '{')) => true,
            Some(Tok::Sym('|')) => self.abs_depth == 0,
            _ => false,
        }
    }

    /// Runs `f` with `|` nesting reset, as brackets start a fresh context.
    fn fresh_abs<T>(&mut self, f: impl FnOnce(&mut Self) -> Res<T>) -> Res<T> {
        let saved = std::mem::take(&mut self.abs_depth);
        let result = f(self);
        self.abs_depth = saved;
        result
    }

    fn parse_expr(&mut self) -> Res<Expr> {
        let mut left = self.parse_term()?;
        loop {
            if self.eat_sym('+') {
                left = Expr::Add(Box::new(left), Box::new(self.parse_term()?));
            } else if self.eat_sym('-') {
                left = Expr::Sub(Box::new(left), Box::new(self.parse_term()?));
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_term(&mut self) -> Res<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Sym('*')) => Some(true),
                Some(Tok::Sym('/')) => Some(false),
                Some(Tok::Cmd(c)) if c == "\\cdot" || c == "\\times" => Some(true),
                Some(Tok::Cmd(c)) if c == "\\div" => Some(false),
                _ => None,
            };
            if let Some(is_mul) = op {
                self.pos += 1;
                let right = self.parse_unary()?;
                left = if is_mul {
                    Expr::Mul(Box::new(left), Box::new(right))
                } else {
                    Expr::Div(Box::new(left), Box::new(right))
                };
            } else if self.starts_value() {
                let right = self.parse_power()?;
                left = Expr::Mul(Box::new(left), Box::new(right));
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_unary(&mut self) -> Res<Expr> {
        if self.eat_sym('-') {
            Ok(Expr::Neg(Box::new(self.parse_unary()?)))
        } else if self.eat_sym('+') {
            self.parse_unary()
        } else {
            self.parse_power()
        }
    }

    fn parse_power(&mut self) -> Res<Expr> {
        let base = self.parse_primary()?;
        self.parse_power_suffix(base)
    }

    fn parse_power_suffix(&mut self, base: Expr) -> Res<Expr> {
        if !self.eat_sym('^') {
            return Ok(base);
        }
        let exponent = self.parse_script()?;
        let exponent = self.parse_power_suffix(exponent)?;
        Ok(Expr::Pow(Box::new(base), Box::new(exponent)))
    }

    /// A TeX argument: a `{group}` or a single token, so `2^12` means `2^1 2`.
    fn parse_script(&mut self) -> Res<Expr> {
        if self.eat_sym('-') {
            return Ok(Expr::Neg(Box::new(self.parse_script()?)));
        }
        if self.eat_sym('+') {
            return self.parse_script();
        }
        if let Some(Tok::Num(_)) = self.peek() {
            return Ok(Expr::Num(self.take_single_digit()));
        }
        self.parse_primary()
    }

    /// Consumes one digit of a number token, leaving the rest for the next read.
    fn take_single_digit(&mut self) -> f64 {
        let token = &mut self.tokens[self.pos];
        let Tok::Num(text) = &token.tok else {
            return f64::NAN;
        };
        if text.starts_with('.') || text.len() == 1 {
            let value = text.parse().unwrap_or(f64::NAN);
            self.pos += 1;
            return value;
        }
        let (first, rest) = text.split_at(1);
        let value = first.parse().unwrap_or(f64::NAN);
        let rest = rest.to_string();
        token.tok = Tok::Num(rest);
        token.span.start += 1;
        value
    }

    fn parse_argument(&mut self, cmd: &str, span: &Range<usize>) -> Res<Expr> {
        let ready =
            self.starts_value() || self.peek_sym('-') || self.peek_sym('+') || self.peek_sym('|');
        if !ready {
            return err(Kind::MissingArgument(cmd.to_string()), span.clone());
        }
        self.parse_script()
    }

    fn parse_primary(&mut self) -> Res<Expr> {
        let Some(tok) = self.peek().cloned() else {
            return err(Kind::ExpectedValue, self.here());
        };
        let span = self.here();
        match tok {
            Tok::Num(text) => {
                self.pos += 1;
                Ok(Expr::Num(text.parse().unwrap_or(f64::NAN)))
            }
            Tok::Ident(c) => {
                self.pos += 1;
                let name = self.with_subscript(c.to_string())?;
                Ok(if name == "e" {
                    Expr::Num(E)
                } else {
                    Expr::Var(name)
                })
            }
            Tok::Sym('(') => {
                self.pos += 1;
                self.parse_group(')', "(", span)
            }
            Tok::Sym('{') => {
                self.pos += 1;
                self.parse_group('}', "{", span)
            }
            Tok::Sym('|') => {
                self.pos += 1;
                self.abs_depth += 1;
                let inner = self.parse_expr();
                self.abs_depth -= 1;
                let inner = inner?;
                self.expect_close('|', "|", span)?;
                Ok(Expr::Call(Func::Abs, Box::new(inner)))
            }
            Tok::Sym(_) => err(Kind::ExpectedValue, span),
            Tok::Cmd(name) => {
                self.pos += 1;
                self.parse_command(&name, span)
            }
        }
    }

    fn parse_group(&mut self, close: char, open: &str, span: Range<usize>) -> Res<Expr> {
        self.fresh_abs(|p| {
            let inner = p.parse_expr()?;
            p.expect_close(close, open, span)?;
            Ok(inner)
        })
    }

    /// Reads `_x`, `_1` or `_{12}` after a name.
    fn with_subscript(&mut self, name: String) -> Res<String> {
        if !self.eat_sym('_') {
            return Ok(name);
        }
        let span = self.here();
        if self.eat_sym('{') {
            let mut sub = String::new();
            loop {
                match self.peek().cloned() {
                    Some(Tok::Ident(c)) => sub.push(c),
                    Some(Tok::Num(t)) => sub.push_str(&t),
                    Some(Tok::Sym('}')) => break,
                    _ => return err(Kind::Unclosed("{".into()), span),
                }
                self.pos += 1;
            }
            self.pos += 1;
            if sub.is_empty() {
                return err(Kind::ExpectedValue, span);
            }
            return Ok(format!("{name}_{sub}"));
        }
        match self.peek().cloned() {
            Some(Tok::Ident(c)) => {
                self.pos += 1;
                Ok(format!("{name}_{c}"))
            }
            Some(Tok::Num(_)) => {
                let digit = self.take_single_digit();
                Ok(format!("{name}_{digit}"))
            }
            _ => err(Kind::ExpectedValue, span),
        }
    }

    fn parse_command(&mut self, name: &str, span: Range<usize>) -> Res<Expr> {
        if let Some(f) = func_from_cmd(name) {
            return self.parse_function(f, name, span);
        }
        match name {
            "\\pi" => Ok(Expr::Num(PI)),
            "\\frac" => {
                let top = self.parse_argument(name, &span)?;
                let bottom = self.parse_argument(name, &span)?;
                Ok(Expr::Div(Box::new(top), Box::new(bottom)))
            }
            "\\sqrt" => self.parse_sqrt(name, span),
            "\\log" => self.parse_log(name, span),
            "\\lfloor" => self.parse_delimited(Func::Floor, "\\rfloor", name, span),
            "\\lceil" => self.parse_delimited(Func::Ceil, "\\rceil", name, span),
            "\\max" | "\\min" => self.parse_min_max(name, span),
            greek if is_greek(greek) => {
                let full = self.with_subscript(greek.to_string())?;
                Ok(Expr::Var(full))
            }
            unknown => err(Kind::UnknownCommand(unknown.to_string()), span),
        }
    }

    fn parse_sqrt(&mut self, name: &str, span: Range<usize>) -> Res<Expr> {
        if self.peek_sym('[') {
            let open = self.here();
            self.pos += 1;
            let degree = self.fresh_abs(|p| {
                let degree = p.parse_expr()?;
                p.expect_close(']', "[", open)?;
                Ok(degree)
            })?;
            let value = self.parse_argument(name, &span)?;
            return Ok(Expr::Root(Box::new(degree), Box::new(value)));
        }
        let value = self.parse_argument(name, &span)?;
        Ok(Expr::Call(Func::Sqrt, Box::new(value)))
    }

    fn parse_log(&mut self, name: &str, span: Range<usize>) -> Res<Expr> {
        let base = if self.eat_sym('_') {
            self.parse_script()?
        } else {
            Expr::Num(10.0)
        };
        let power = self.parse_optional_superscript()?;
        let value = self.parse_function_arg(name, &span)?;
        let log = Expr::Log(Box::new(base), Box::new(value));
        Ok(wrap_power(log, power))
    }

    fn parse_delimited(
        &mut self,
        f: Func,
        close: &str,
        open: &str,
        span: Range<usize>,
    ) -> Res<Expr> {
        self.fresh_abs(|p| {
            let inner = p.parse_expr()?;
            if p.peek() == Some(&Tok::Cmd(close.to_string())) {
                p.pos += 1;
                Ok(Expr::Call(f, Box::new(inner)))
            } else {
                err(Kind::Unclosed(open.to_string()), span)
            }
        })
    }

    fn parse_min_max(&mut self, name: &str, span: Range<usize>) -> Res<Expr> {
        let open = self.here();
        if !self.eat_sym('(') {
            return err(Kind::MissingArgument(name.to_string()), span);
        }
        self.fresh_abs(|p| {
            let a = p.parse_expr()?;
            if !p.eat_sym(',') {
                return err(Kind::MissingArgument(name.to_string()), span);
            }
            let b = p.parse_expr()?;
            p.expect_close(')', "(", open)?;
            Ok(if name == "\\max" {
                Expr::Max(Box::new(a), Box::new(b))
            } else {
                Expr::Min(Box::new(a), Box::new(b))
            })
        })
    }

    fn parse_optional_superscript(&mut self) -> Res<Option<Expr>> {
        if self.eat_sym('^') {
            Ok(Some(self.parse_script()?))
        } else {
            Ok(None)
        }
    }

    fn parse_function(&mut self, f: Func, name: &str, span: Range<usize>) -> Res<Expr> {
        let power = self.parse_optional_superscript()?;
        let (f, power) = match (&power, inverse_of(f)) {
            (Some(p), Some(inverse)) if is_minus_one(p) => (inverse, None),
            _ => (f, power),
        };
        let arg = self.parse_function_arg(name, &span)?;
        Ok(wrap_power(Expr::Call(f, Box::new(arg)), power))
    }

    /// A bracketed argument ends at its bracket. Without brackets the argument
    /// extends over the following numbers and names: `\sin 2x`, `\sin x^2`.
    fn parse_function_arg(&mut self, name: &str, span: &Range<usize>) -> Res<Expr> {
        if self.peek_sym('(') || self.peek_sym('{') {
            return self.parse_primary();
        }
        if self.eat_sym('-') {
            return Ok(Expr::Neg(Box::new(self.parse_function_arg(name, span)?)));
        }
        if !self.starts_value() && !self.peek_sym('|') {
            return err(Kind::MissingArgument(name.to_string()), span.clone());
        }
        let mut arg = self.parse_power()?;
        while self.continues_bare_argument() {
            let next = self.parse_power()?;
            arg = Expr::Mul(Box::new(arg), Box::new(next));
        }
        Ok(arg)
    }

    fn continues_bare_argument(&self) -> bool {
        match self.peek() {
            Some(Tok::Num(_) | Tok::Ident(_)) => true,
            Some(Tok::Cmd(name)) => name == "\\pi" || is_greek(name),
            _ => false,
        }
    }
}

fn is_minus_one(e: &Expr) -> bool {
    matches!(e, Expr::Neg(inner) if **inner == Expr::Num(1.0))
}

fn wrap_power(base: Expr, power: Option<Expr>) -> Expr {
    match power {
        Some(p) => Expr::Pow(Box::new(base), Box::new(p)),
        None => base,
    }
}

fn parse_tokens(tokens: Vec<Token>, end: usize) -> Res<Expr> {
    let mut parser = Parser::new(tokens, end);
    let expr = parser.parse_expr()?;
    match parser.peek().cloned() {
        None => Ok(expr),
        Some(Tok::Sym(c @ (')' | '}' | ']'))) => {
            err(Kind::UnmatchedClose(c.to_string()), parser.here())
        }
        Some(Tok::Cmd(c)) if c == "\\rfloor" || c == "\\rceil" => {
            err(Kind::UnmatchedClose(c), parser.here())
        }
        Some(Tok::Sym(c)) => err(Kind::Trailing(c.to_string()), parser.here()),
        Some(Tok::Num(t)) => err(Kind::Trailing(t), parser.here()),
        Some(Tok::Ident(c)) => err(Kind::Trailing(c.to_string()), parser.here()),
        Some(Tok::Cmd(c)) => err(Kind::Trailing(c), parser.here()),
    }
}

/// Parses a plain expression with no `y=` prefix, such as an integration bound.
pub fn parse_expression(src: &str) -> Res<Expr> {
    let tokens = lex(src)?;
    if tokens.is_empty() {
        return err(Kind::Empty, 0..src.len());
    }
    parse_tokens(tokens, src.len())
}

/// Parses a full formula: `y=...`, `f(x)=...`, `r=...`, `(x(t), y(t))` or a bare expression.
pub fn parse(src: &str) -> Res<Formula> {
    let tokens = lex(src)?;
    if tokens.is_empty() {
        return err(Kind::Empty, 0..src.len());
    }
    let end = src.len();
    let (kind, body_tokens) = split_lhs(src, &tokens)?;
    let kind = match kind {
        Lhs::Cartesian => {
            if let Some((x, y)) = split_tuple(body_tokens)? {
                FormulaKind::Parametric {
                    x: parse_slice(x, end)?,
                    y: parse_slice(y, end)?,
                }
            } else {
                FormulaKind::Cartesian(parse_slice(body_tokens, end)?)
            }
        }
        Lhs::Polar => FormulaKind::Polar(parse_slice(body_tokens, end)?),
    };
    finish(kind, &tokens)
}

fn parse_slice(tokens: &[Token], end: usize) -> Res<Expr> {
    if tokens.is_empty() {
        return err(Kind::ExpectedValue, end..end);
    }
    parse_tokens(tokens.to_vec(), end)
}

enum Lhs {
    Cartesian,
    Polar,
}

fn split_lhs<'a>(src: &str, tokens: &'a [Token]) -> Res<(Lhs, &'a [Token])> {
    let mut equals = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.tok == Tok::Sym('='));
    let Some((at, _)) = equals.next() else {
        return Ok((Lhs::Cartesian, tokens));
    };
    if let Some((_, second)) = equals.next() {
        return err(Kind::Trailing("=".into()), second.span.clone());
    }
    let (lhs, rhs) = (&tokens[..at], &tokens[at + 1..]);
    let is_call = |arg: &Tok| {
        lhs.len() == 4
            && matches!(lhs[0].tok, Tok::Ident(_))
            && lhs[1].tok == Tok::Sym('(')
            && lhs[2].tok == *arg
            && lhs[3].tok == Tok::Sym(')')
    };
    let single = |c: char| lhs.len() == 1 && lhs[0].tok == Tok::Ident(c);
    if single('y') || is_call(&Tok::Ident('x')) {
        Ok((Lhs::Cartesian, rhs))
    } else if single('r') || is_call(&Tok::Cmd(VAR_THETA.to_string())) {
        Ok((Lhs::Polar, rhs))
    } else {
        let span = lhs.first().map_or(0, |t| t.span.start)..tokens[at].span.start;
        let text = src[span.clone()].trim().to_string();
        err(Kind::UnsupportedLhs(text), span)
    }
}

/// Recognises `( a , b )` spanning the whole slice, ignoring nested commas.
fn split_tuple(tokens: &[Token]) -> Res<Option<(&[Token], &[Token])>> {
    let Some(first) = tokens.first() else {
        return Ok(None);
    };
    if first.tok != Tok::Sym('(') {
        return Ok(None);
    }
    let mut depth = 0usize;
    let mut commas = Vec::new();
    let mut close_at = None;
    for (i, t) in tokens.iter().enumerate() {
        match t.tok {
            Tok::Sym('(' | '{' | '[') => depth += 1,
            Tok::Sym(')' | '}' | ']') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    close_at = Some(i);
                    break;
                }
            }
            Tok::Sym(',') if depth == 1 => commas.push(i),
            _ => {}
        }
    }
    let Some(close_at) = close_at else {
        return Ok(None);
    };
    if close_at != tokens.len() - 1 || commas.is_empty() {
        return Ok(None);
    }
    if commas.len() != 1 {
        return err(Kind::BadTuple, first.span.start..tokens[close_at].span.end);
    }
    Ok(Some((
        &tokens[1..commas[0]],
        &tokens[commas[0] + 1..close_at],
    )))
}

fn finish(kind: FormulaKind, tokens: &[Token]) -> Res<Formula> {
    let mut vars = BTreeSet::new();
    let reserved: &[&str] = match &kind {
        FormulaKind::Cartesian(e) => {
            e.collect_vars(&mut vars);
            &["y"]
        }
        FormulaKind::Parametric { x, y } => {
            x.collect_vars(&mut vars);
            y.collect_vars(&mut vars);
            &["x", "y"]
        }
        FormulaKind::Polar(e) => {
            e.collect_vars(&mut vars);
            &["x", "y", "r"]
        }
    };
    for name in reserved {
        if vars.contains(*name) {
            let letter = name.chars().next().unwrap_or('x');
            // A formula's left side uses y/r too, so point at the last use.
            let span = tokens
                .iter()
                .rev()
                .find(|t| t.tok == Tok::Ident(letter))
                .map_or(0..0, |t| t.span.clone());
            return err(Kind::ReservedVariable((*name).to_string()), span);
        }
    }
    vars.remove(kind.main_var());
    Ok(Formula {
        kind,
        params: vars.into_iter().collect(),
    })
}
