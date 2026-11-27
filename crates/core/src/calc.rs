//! Expression calculator engine behind the Calculator tool.
//!
//! The expression is kept as plain ASCII (`*` and `/` for times and
//! divide; the tiny font draws them as × and ÷) and parsed by recursive
//! descent on demand:
//!
//! ```text
//! expr  := term (('+' | '-') term)*
//! term  := unary (('*' | '/') unary | implicit-multiply unary)*
//! unary := '-' unary | power
//! power := atom ('^' unary)?          right-associative
//! atom  := number | '(' expr ')'?     unclosed '(' close at the end
//! ```
//!
//! So `-2^2` is -4, `2^3^2` is 512 and `2(3)` is 6. Everything is f64;
//! `pow`, `exp` and `ln` are implemented here because `no_std` has no
//! float math and micromath is f32 and approximate.

use core::fmt::Write;
use heapless::String;

/// Longest expression the player can type.
pub const MAX_EXPR: usize = 40;
/// Formatted results fit the 2x display row.
pub const MAX_RESULT: usize = 12;
/// Parenthesis nesting limit, to keep the recursive parser's stack small.
pub const MAX_DEPTH: u8 = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CalcError {
    Syntax,
    DivideByZero,
    /// No real answer, e.g. a fractional power of a negative number.
    Domain,
    Overflow,
    TooDeep,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Digit(u8),
    Dot,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Open,
    Close,
    Clear,
    Equals,
}

/// Typing state: the expression, plus the answer once `=` is pressed.
pub struct Calculator {
    expr: String<MAX_EXPR>,
    /// `Some` right after `=`, until the next edit.
    evaluated: Option<Result<f64, CalcError>>,
    /// What the result row shows: the answer after `=`, otherwise a live
    /// preview of the expression so far (blank while it doesn't parse).
    shown: String<MAX_RESULT>,
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Calculator {
    pub const fn new() -> Self {
        Self { expr: String::new(), evaluated: None, shown: String::new() }
    }

    pub fn expr(&self) -> &str {
        self.expr.as_str()
    }

    pub fn result_text(&self) -> &str {
        self.shown.as_str()
    }

    /// True right after `=`, so the result row can be drawn as final.
    pub fn is_evaluated(&self) -> bool {
        self.evaluated.is_some()
    }

    pub fn clear(&mut self) {
        self.expr.clear();
        self.evaluated = None;
        self.shown.clear();
    }

    pub fn backspace(&mut self) {
        self.evaluated = None;
        self.expr.pop();
        self.refresh_preview();
    }

    pub fn press(&mut self, key: Key) {
        if let Some(prev) = self.evaluated.take() {
            match key {
                Key::Equals => {
                    self.evaluated = Some(prev);
                    return;
                }
                // An operator carries on from the answer.
                Key::Add | Key::Sub | Key::Mul | Key::Div | Key::Pow => {
                    self.expr.clear();
                    if let Ok(v) = prev {
                        let _ = self.expr.push_str(format_number(v).as_str());
                    }
                }
                // Anything else starts a fresh expression.
                _ => self.expr.clear(),
            }
        }

        match key {
            Key::Digit(d) => {
                self.push((b'0' + d.min(9)) as char);
            }
            Key::Dot => {
                if !self.current_number_has_dot() {
                    if !self.last().is_some_and(|c| c.is_ascii_digit()) {
                        self.push('0');
                    }
                    self.push('.');
                }
            }
            Key::Add => self.push_binary('+'),
            Key::Mul => self.push_binary('*'),
            Key::Div => self.push_binary('/'),
            Key::Pow => self.push_binary('^'),
            Key::Sub => {
                // Replace a trailing + or - rather than stacking signs.
                // After `*`, `/`, `^`, `(` or at the start it's unary.
                if matches!(self.last(), Some('+') | Some('-')) {
                    self.expr.pop();
                }
                self.push('-');
            }
            Key::Open => self.push('('),
            Key::Close => {
                let ends_value = self.last().is_some_and(|c| c.is_ascii_digit() || c == '.' || c == ')');
                if self.open_parens() > 0 && ends_value {
                    self.push(')');
                }
            }
            Key::Clear => {
                self.clear();
                return;
            }
            Key::Equals => {
                if self.expr.is_empty() {
                    return;
                }
                let result = evaluate(self.expr.as_str());
                self.shown = match result {
                    Ok(v) => format_number(v),
                    Err(_) => err_text(),
                };
                self.evaluated = Some(result);
                return;
            }
        }
        self.refresh_preview();
    }

    fn refresh_preview(&mut self) {
        self.shown = match evaluate(self.expr.as_str()) {
            Ok(v) => format_number(v),
            Err(_) => String::new(),
        };
    }

    fn push(&mut self, c: char) {
        let _ = self.expr.push(c);
    }

    fn last(&self) -> Option<char> {
        self.expr.chars().last()
    }

    /// A binary operator needs a value on its left. Pressing one straight
    /// after another operator swaps it in.
    fn push_binary(&mut self, op: char) {
        while self.last().is_some_and(is_op) {
            self.expr.pop();
        }
        match self.last() {
            None | Some('(') => {
                // Nothing to operate on; dropping trailing ops may have
                // removed a unary minus, which is fine.
                self.refresh_preview();
            }
            _ => self.push(op),
        }
    }

    fn current_number_has_dot(&self) -> bool {
        for c in self.expr.chars().rev() {
            if c == '.' {
                return true;
            }
            if !c.is_ascii_digit() {
                return false;
            }
        }
        false
    }

    fn open_parens(&self) -> i32 {
        self.expr.chars().fold(0, |n, c| match c {
            '(' => n + 1,
            ')' => n - 1,
            _ => n,
        })
    }
}

fn is_op(c: char) -> bool {
    matches!(c, '+' | '-' | '*' | '/' | '^')
}

fn err_text() -> String<MAX_RESULT> {
    let mut s = String::new();
    let _ = s.push_str("Err");
    s
}

// ---------------------------------------------------------------------------
// Parser.
// ---------------------------------------------------------------------------

/// Evaluate an ASCII expression. Unclosed parentheses are closed at the end.
pub fn evaluate(expr: &str) -> Result<f64, CalcError> {
    let mut p = Parser { s: expr.as_bytes(), i: 0, depth: 0 };
    if p.s.is_empty() {
        return Err(CalcError::Syntax);
    }
    let v = p.expr()?;
    if p.i != p.s.len() {
        return Err(CalcError::Syntax);
    }
    if !v.is_finite() {
        return Err(CalcError::Overflow);
    }
    // Normalise -0 so it prints as 0.
    Ok(if v == 0.0 { 0.0 } else { v })
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: u8,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn expr(&mut self) -> Result<f64, CalcError> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.i += 1;
                    v += self.term()?;
                }
                Some(b'-') => {
                    self.i += 1;
                    v -= self.term()?;
                }
                _ => return Ok(v),
            }
        }
    }

    fn term(&mut self) -> Result<f64, CalcError> {
        let mut v = self.unary()?;
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.i += 1;
                    v *= self.unary()?;
                }
                Some(b'/') => {
                    self.i += 1;
                    let d = self.unary()?;
                    if d == 0.0 {
                        return Err(CalcError::DivideByZero);
                    }
                    v /= d;
                }
                // A value straight after a value: `2(3)`, `(1)(2)`, `(1)2`.
                Some(b'(') | Some(b'0'..=b'9') | Some(b'.') => {
                    v *= self.unary()?;
                }
                _ => return Ok(v),
            }
        }
    }

    fn unary(&mut self) -> Result<f64, CalcError> {
        if self.peek() == Some(b'-') {
            self.i += 1;
            return Ok(-self.unary()?);
        }
        self.power()
    }

    fn power(&mut self) -> Result<f64, CalcError> {
        let base = self.atom()?;
        if self.peek() == Some(b'^') {
            self.i += 1;
            let exp = self.unary()?;
            return pow(base, exp);
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<f64, CalcError> {
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return Err(CalcError::TooDeep);
                }
                let v = self.expr()?;
                match self.peek() {
                    Some(b')') => self.i += 1,
                    None => {} // auto-close
                    Some(_) => return Err(CalcError::Syntax),
                }
                self.depth -= 1;
                Ok(v)
            }
            Some(b'0'..=b'9') | Some(b'.') => self.number(),
            _ => Err(CalcError::Syntax),
        }
    }

    /// Digits with an optional fraction and an optional `e` exponent (the
    /// exponent only appears when carrying on from a formatted answer).
    fn number(&mut self) -> Result<f64, CalcError> {
        let mut mantissa: f64 = 0.0;
        let mut scale: i32 = 0;
        let mut any_digit = false;
        while let Some(c @ b'0'..=b'9') = self.peek() {
            mantissa = mantissa * 10.0 + (c - b'0') as f64;
            any_digit = true;
            self.i += 1;
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            while let Some(c @ b'0'..=b'9') = self.peek() {
                mantissa = mantissa * 10.0 + (c - b'0') as f64;
                scale -= 1;
                any_digit = true;
                self.i += 1;
            }
        }
        if !any_digit {
            return Err(CalcError::Syntax);
        }
        if self.peek() == Some(b'e') {
            self.i += 1;
            let neg = match self.peek() {
                Some(b'-') => {
                    self.i += 1;
                    true
                }
                Some(b'+') => {
                    self.i += 1;
                    false
                }
                _ => false,
            };
            let mut e: i32 = 0;
            let mut any = false;
            while let Some(c @ b'0'..=b'9') = self.peek() {
                e = (e * 10 + (c - b'0') as i32).min(10_000);
                any = true;
                self.i += 1;
            }
            if !any {
                return Err(CalcError::Syntax);
            }
            scale += if neg { -e } else { e };
        }
        Ok(mantissa * pow10(scale))
    }
}

// ---------------------------------------------------------------------------
// Float math.
// ---------------------------------------------------------------------------

const LN2: f64 = core::f64::consts::LN_2;

fn pow10(n: i32) -> f64 {
    // Dividing by an exact power of ten keeps 0.1 as close as a literal.
    if n >= 0 {
        powi(10.0, n as i64)
    } else {
        1.0 / powi(10.0, -(n as i64))
    }
}

/// `b^e` by repeated squaring, for `e >= 0`.
fn powi(mut b: f64, mut e: i64) -> f64 {
    let mut acc = 1.0;
    while e > 0 {
        if e & 1 == 1 {
            acc *= b;
        }
        b *= b;
        e >>= 1;
    }
    acc
}

/// Real power. Exact by squaring for whole exponents; `exp(e * ln b)`
/// otherwise.
pub fn pow(b: f64, e: f64) -> Result<f64, CalcError> {
    if e == 0.0 {
        return Ok(1.0);
    }
    let whole = e.abs() < 1e15 && (e as i64) as f64 == e;
    if b == 0.0 {
        return if e > 0.0 { Ok(0.0) } else { Err(CalcError::DivideByZero) };
    }
    let v = if whole {
        let n = e as i64;
        let r = powi(b, n.abs());
        if n < 0 { 1.0 / r } else { r }
    } else {
        if b < 0.0 {
            return Err(CalcError::Domain);
        }
        exp(e * ln(b))
    };
    if v.is_finite() {
        Ok(v)
    } else {
        Err(CalcError::Overflow)
    }
}

/// 2^n for any n a finite f64 result can need.
fn pow2(n: i32) -> f64 {
    let half = n / 2;
    let one = |k: i32| f64::from_bits(((k + 1023) as u64) << 52);
    one(half) * one(n - half)
}

/// Natural log for `x > 0`: split off the binary exponent, then an atanh
/// series on the mantissa, which is kept within [sqrt(1/2), sqrt(2)).
fn ln(x: f64) -> f64 {
    let mut x = x;
    let mut k: i32 = 0;
    if x < f64::MIN_POSITIVE {
        x *= pow2(60);
        k -= 60;
    }
    let bits = x.to_bits();
    k += ((bits >> 52) & 0x7ff) as i32 - 1023;
    let mut m = f64::from_bits((bits & ((1u64 << 52) - 1)) | (1023u64 << 52));
    if m > core::f64::consts::SQRT_2 {
        m *= 0.5;
        k += 1;
    }
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let mut term = s;
    let mut sum = 0.0;
    let mut n = 1.0;
    for _ in 0..40 {
        let add = term / n;
        sum += add;
        if add.abs() < 1e-18 {
            break;
        }
        term *= s2;
        n += 2.0;
    }
    k as f64 * LN2 + 2.0 * sum
}

/// e^y: reduce to `k*ln2 + r` with |r| <= ln2/2, then a Taylor series.
fn exp(y: f64) -> f64 {
    if y > 709.8 {
        return f64::INFINITY;
    }
    if y < -745.2 {
        return 0.0;
    }
    let kf = y / LN2;
    let k = if kf >= 0.0 { (kf + 0.5) as i32 } else { (kf - 0.5) as i32 };
    let r = y - k as f64 * LN2;
    let mut term = 1.0;
    let mut sum = 1.0;
    for i in 1..30 {
        term *= r / i as f64;
        sum += term;
        if term.abs() < 1e-18 {
            break;
        }
    }
    sum * pow2(k)
}

// ---------------------------------------------------------------------------
// Formatting.
// ---------------------------------------------------------------------------

/// Format `v` in at most [`MAX_RESULT`] characters: plain decimal when it
/// fits, otherwise `1.25e-15` style, keeping as many significant digits
/// (up to 12) as the width allows. Trailing zeros are trimmed, which also
/// hides float noise like 0.1 + 0.2 = 0.30000000000000004.
pub fn format_number(v: f64) -> String<MAX_RESULT> {
    if !v.is_finite() {
        return err_text();
    }
    if v == 0.0 {
        let mut s = String::new();
        let _ = s.push('0');
        return s;
    }
    for prec in (0..=11).rev() {
        let (neg, digits, e) = sci_parts(v, prec);
        if let Some(s) = fixed(neg, digits.as_str(), e) {
            return s;
        }
    }
    for prec in (0..=11).rev() {
        let (neg, digits, e) = sci_parts(v, prec);
        if let Some(s) = scientific(neg, digits.as_str(), e) {
            return s;
        }
    }
    err_text()
}

/// Round `v` to `prec + 1` significant digits. Returns the sign, the
/// digits without trailing zeros, and the decimal exponent of the first.
fn sci_parts(v: f64, prec: usize) -> (bool, String<16>, i32) {
    let mut buf: String<32> = String::new();
    let _ = write!(buf, "{:.*e}", prec, v.abs());
    let (mant, exp) = buf.split_once('e').unwrap_or((buf.as_str(), "0"));
    let mut digits: String<16> = String::new();
    for c in mant.chars().filter(|c| c.is_ascii_digit()) {
        let _ = digits.push(c);
    }
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    (v < 0.0, digits, exp.parse().unwrap_or(0))
}

fn fixed(neg: bool, digits: &str, e: i32) -> Option<String<MAX_RESULT>> {
    let mut s: String<MAX_RESULT> = String::new();
    if neg {
        s.push('-').ok()?;
    }
    let n = digits.len() as i32;
    if e >= 0 {
        if e >= MAX_RESULT as i32 {
            return None;
        }
        for i in 0..=e {
            let c = if i < n { digits.as_bytes()[i as usize] as char } else { '0' };
            s.push(c).ok()?;
        }
        if n > e + 1 {
            s.push('.').ok()?;
            s.push_str(&digits[(e + 1) as usize..]).ok()?;
        }
    } else {
        s.push_str("0.").ok()?;
        for _ in 0..(-e - 1) {
            s.push('0').ok()?;
        }
        s.push_str(digits).ok()?;
    }
    Some(s)
}

fn scientific(neg: bool, digits: &str, e: i32) -> Option<String<MAX_RESULT>> {
    let mut s: String<MAX_RESULT> = String::new();
    if neg {
        s.push('-').ok()?;
    }
    s.push_str(&digits[..1]).ok()?;
    if digits.len() > 1 {
        s.push('.').ok()?;
        s.push_str(&digits[1..]).ok()?;
    }
    write!(s, "e{}", e).ok()?;
    Some(s)
}
