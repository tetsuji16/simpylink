//! Numeric parameter expressions (subset of MATLAB literal syntax).
use crate::workspace::Workspace;

/// Row-major matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl Matrix {
    /// Element (r, c).
    pub fn get(&self, r: usize, c: usize) -> f64 {
        self.data[r * self.cols + c]
    }
    /// Empty 0x0 matrix.
    pub fn empty() -> Matrix {
        Matrix {
            rows: 0,
            cols: 0,
            data: vec![],
        }
    }
}

/// Result of evaluating an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Scalar(f64),
    Matrix(Matrix),
}

impl Value {
    /// Scalar, or a 1x1 matrix.
    pub fn as_scalar(&self) -> Option<f64> {
        match self {
            Value::Scalar(v) => Some(*v),
            Value::Matrix(m) if m.rows == 1 && m.cols == 1 => Some(m.data[0]),
            _ => None,
        }
    }
    /// Scalar → [v]; 1xN / Nx1 → elements; [] → empty.
    pub fn as_vector(&self) -> Option<Vec<f64>> {
        match self {
            Value::Scalar(v) => Some(vec![*v]),
            Value::Matrix(m) if m.rows <= 1 || m.cols <= 1 => Some(m.data.clone()),
            _ => None,
        }
    }
    /// Always a matrix (scalar → 1x1).
    pub fn as_matrix(&self) -> Matrix {
        match self {
            Value::Scalar(v) => Matrix {
                rows: 1,
                cols: 1,
                data: vec![*v],
            },
            Value::Matrix(m) => m.clone(),
        }
    }
}

/// Expression error with byte position.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message} (at byte {pos})")]
pub struct ExprError {
    pub message: String,
    pub pos: usize,
}

fn err<T>(message: impl Into<String>, pos: usize) -> Result<T, ExprError> {
    Err(ExprError {
        message: message.into(),
        pos,
    })
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    DotStar,
    DotSlash,
    DotCaret,
    Quote,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semi,
    End,
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    pos: usize,
    space_before: bool,
}

fn lex(src: &str) -> Result<Vec<Token>, ExprError> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = vec![];
    let mut space = false;
    while i < b.len() {
        let c = b[i];
        if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
            space = true;
            i += 1;
            continue;
        }
        let start = i;
        let tok =
            if c.is_ascii_digit() || (c == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                if i < b.len() && b[i] == b'.' {
                    i += 1;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                    let mut j = i + 1;
                    if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                        j += 1;
                    }
                    if j < b.len() && b[j].is_ascii_digit() {
                        while j < b.len() && b[j].is_ascii_digit() {
                            j += 1;
                        }
                        i = j;
                    } else {
                        return err("invalid number", start);
                    }
                }
                match src[start..i].parse::<f64>() {
                    Ok(v) => Tok::Num(v),
                    Err(_) => return err("invalid number", start),
                }
            } else if c.is_ascii_alphabetic() || c == b'_' {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                while i < b.len()
                    && b[i] == b'.'
                    && i + 1 < b.len()
                    && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_')
                {
                    i += 1;
                    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                        i += 1;
                    }
                }
                Tok::Ident(src[start..i].to_string())
            } else if c == b'.'
                && i + 1 < b.len()
                && (b[i + 1] == b'*' || b[i + 1] == b'/' || b[i + 1] == b'^')
            {
                i += 2;
                match b[start + 1] {
                    b'*' => Tok::DotStar,
                    b'/' => Tok::DotSlash,
                    b'^' => Tok::DotCaret,
                    _ => unreachable!(),
                }
            } else {
                i += 1;
                match c {
                    b'+' => Tok::Plus,
                    b'-' => Tok::Minus,
                    b'*' => Tok::Star,
                    b'/' => Tok::Slash,
                    b'^' => Tok::Caret,
                    b'\'' => Tok::Quote,
                    b'(' => Tok::LParen,
                    b')' => Tok::RParen,
                    b'[' => Tok::LBracket,
                    b']' => Tok::RBracket,
                    b',' => Tok::Comma,
                    b';' => Tok::Semi,
                    _ => {
                        let ch = src[start..].chars().next().unwrap_or('?');
                        return err(format!("unexpected character '{ch}'"), start);
                    }
                }
            };
        out.push(Token {
            tok,
            pos: start,
            space_before: space,
        });
        space = false;
    }
    out.push(Token {
        tok: Tok::End,
        pos: src.len(),
        space_before: space,
    });
    Ok(out)
}

struct Parser<'w> {
    toks: Vec<Token>,
    ws: &'w Workspace,
    i: usize,
    bracket_depth: usize,
    paren_depth: usize,
    /// Current nesting of `parse_unary`; every recursive cycle passes through it.
    depth: usize,
}

/// Deepest nesting (parentheses, brackets, unary signs, `^` chains) an expression may have.
///
/// The parser is recursive, so an unbounded depth would overflow the stack on a
/// hostile parameter string such as 100 000 opening parentheses.
pub const MAX_EXPR_DEPTH: usize = 64;

impl Parser<'_> {
    fn peek(&self) -> &Token {
        &self.toks[self.i]
    }
    fn peek_at(&self, k: usize) -> &Token {
        &self.toks[(self.i + k).min(self.toks.len() - 1)]
    }
    fn next(&mut self) -> Token {
        let t = self.toks[self.i].clone();
        if self.i < self.toks.len() - 1 {
            self.i += 1;
        }
        t
    }
    fn in_matrix(&self) -> bool {
        self.bracket_depth > 0 && self.paren_depth == 0
    }
    /// Inside a matrix, does the next token start a new element?
    fn is_element_boundary(&self) -> bool {
        if !self.in_matrix() {
            return false;
        }
        let t = self.peek();
        match t.tok {
            Tok::Plus | Tok::Minus => t.space_before && !self.peek_at(1).space_before,
            Tok::Num(_) | Tok::Ident(_) | Tok::LParen | Tok::LBracket => t.space_before,
            _ => false,
        }
    }
    fn parse_expr(&mut self) -> Result<Value, ExprError> {
        let mut v = self.parse_term()?;
        loop {
            if self.is_element_boundary() {
                break;
            }
            let t = self.peek().clone();
            match t.tok {
                Tok::Plus | Tok::Minus => {
                    self.next();
                    let r = self.parse_term()?;
                    v = binop(&v, &r, if t.tok == Tok::Plus { '+' } else { '-' }, t.pos)?;
                }
                _ => break,
            }
        }
        Ok(v)
    }
    fn parse_term(&mut self) -> Result<Value, ExprError> {
        let mut v = self.parse_unary()?;
        loop {
            if self.is_element_boundary() {
                break;
            }
            let t = self.peek().clone();
            match t.tok {
                Tok::Star | Tok::Slash | Tok::DotStar | Tok::DotSlash => {
                    self.next();
                    let r = self.parse_unary()?;
                    let op = if t.tok == Tok::Star || t.tok == Tok::DotStar {
                        '*'
                    } else {
                        '/'
                    };
                    v = binop(&v, &r, op, t.pos)?;
                }
                _ => break,
            }
        }
        Ok(v)
    }
    fn parse_unary(&mut self) -> Result<Value, ExprError> {
        if self.depth >= MAX_EXPR_DEPTH {
            return err("expression is nested too deeply", self.peek().pos);
        }
        self.depth += 1;
        let v = self.parse_unary_inner();
        self.depth -= 1;
        v
    }
    fn parse_unary_inner(&mut self) -> Result<Value, ExprError> {
        let t = self.peek().clone();
        match t.tok {
            Tok::Minus => {
                self.next();
                let v = self.parse_unary()?;
                match v {
                    Value::Scalar(x) => Ok(Value::Scalar(-x)),
                    _ => err("matrix arithmetic is not supported", t.pos),
                }
            }
            Tok::Plus => {
                self.next();
                self.parse_unary()
            }
            _ => self.parse_power(),
        }
    }
    fn parse_power(&mut self) -> Result<Value, ExprError> {
        let base = self.parse_primary()?;
        if (self.peek().tok == Tok::Caret || self.peek().tok == Tok::DotCaret)
            && !self.is_element_boundary()
        {
            let t = self.next();
            let e = self.parse_unary()?;
            return binop(&base, &e, '^', t.pos);
        }
        Ok(base)
    }
    fn parse_primary(&mut self) -> Result<Value, ExprError> {
        let mut v = self.parse_primary_atom()?;
        while self.peek().tok == Tok::Quote {
            self.next();
            v = match v {
                Value::Scalar(s) => Value::Scalar(s),
                Value::Matrix(m) => {
                    let mut data = vec![0.0; m.rows * m.cols];
                    for r in 0..m.rows {
                        for c in 0..m.cols {
                            data[c * m.rows + r] = m.get(r, c);
                        }
                    }
                    Value::Matrix(Matrix {
                        rows: m.cols,
                        cols: m.rows,
                        data,
                    })
                }
            };
        }
        Ok(v)
    }
    fn parse_primary_atom(&mut self) -> Result<Value, ExprError> {
        let t = self.next();
        match t.tok {
            Tok::Num(v) => Ok(Value::Scalar(v)),
            Tok::Ident(ref s) => {
                if self.peek().tok == Tok::LParen && !self.is_element_boundary() {
                    if self.ws.get(s).is_some() {
                        return err("indexing is not supported", t.pos);
                    }
                    self.parse_function_call(s, t.pos)
                } else if let Some(v) = self.ws.get(s) {
                    Ok(v.clone())
                } else {
                    match s.as_str() {
                        "pi" => Ok(Value::Scalar(std::f64::consts::PI)),
                        "inf" | "Inf" => Ok(Value::Scalar(f64::INFINITY)),
                        "eps" => Ok(Value::Scalar(f64::EPSILON)),
                        _ => err(format!("unknown identifier '{s}'"), t.pos),
                    }
                }
            }
            Tok::LParen => {
                self.paren_depth += 1;
                let v = self.parse_expr()?;
                self.paren_depth -= 1;
                if self.peek().tok != Tok::RParen {
                    return err("expected ')'", self.peek().pos);
                }
                self.next();
                Ok(v)
            }
            Tok::LBracket => self.parse_matrix(t.pos),
            Tok::End => err("unexpected end of expression", t.pos),
            _ => err("unexpected token", t.pos),
        }
    }
    fn parse_function_call(&mut self, name: &str, pos: usize) -> Result<Value, ExprError> {
        self.next(); // consume LParen
        self.paren_depth += 1;
        let mut args: Vec<f64> = vec![];
        if self.peek().tok != Tok::RParen {
            loop {
                let arg_pos = self.peek().pos;
                let val = self.parse_expr()?;
                let scalar = val.as_scalar().ok_or_else(|| ExprError {
                    message: format!("function '{name}' expects scalar arguments"),
                    pos: arg_pos,
                })?;
                args.push(scalar);
                if self.peek().tok == Tok::Comma {
                    self.next();
                    continue;
                }
                break;
            }
        }
        if self.peek().tok != Tok::RParen {
            return err("expected ')'", self.peek().pos);
        }
        self.next(); // consume RParen
        self.paren_depth -= 1;

        eval_function(name, &args, pos)
    }
    fn parse_matrix(&mut self, open_pos: usize) -> Result<Value, ExprError> {
        let saved_paren = self.paren_depth;
        self.paren_depth = 0;
        self.bracket_depth += 1;
        let mut rows: Vec<Vec<f64>> = vec![];
        let mut row: Vec<f64> = vec![];
        loop {
            let t = self.peek().clone();
            match t.tok {
                Tok::RBracket => {
                    self.next();
                    break;
                }
                Tok::Semi => {
                    self.next();
                    rows.push(std::mem::take(&mut row));
                    continue;
                }
                Tok::Comma => {
                    self.next();
                    continue;
                }
                Tok::End => return err("expected ']'", open_pos),
                Tok::LBracket => return err("nested matrices are not supported", t.pos),
                _ => {}
            }
            let v = self.parse_expr()?;
            match v {
                Value::Scalar(x) => row.push(x),
                Value::Matrix(_) => return err("nested matrices are not supported", t.pos),
            }
        }
        if !row.is_empty() || !rows.is_empty() {
            rows.push(row);
        }
        self.bracket_depth -= 1;
        self.paren_depth = saved_paren;
        // drop fully empty trailing rows like "[1;]"
        rows.retain(|r| !r.is_empty());
        if rows.is_empty() {
            return Ok(Value::Matrix(Matrix::empty()));
        }
        let cols = rows[0].len();
        if rows.iter().any(|r| r.len() != cols) {
            return err("rows have different lengths", open_pos);
        }
        let n = rows.len();
        Ok(Value::Matrix(Matrix {
            rows: n,
            cols,
            data: rows.into_iter().flatten().collect(),
        }))
    }
}

const ONE_ARG_FUNCS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "exp", "log", "log10",
    "log2", "sqrt", "abs", "sign", "floor", "ceil", "round", "fix",
];

const TWO_ARG_FUNCS: &[&str] = &["atan2", "mod", "rem", "min", "max", "hypot", "power"];

fn eval_function(name: &str, args: &[f64], pos: usize) -> Result<Value, ExprError> {
    if ONE_ARG_FUNCS.contains(&name) {
        if args.len() != 1 {
            return err(format!("function '{name}' expects 1 argument"), pos);
        }
        let x = args[0];
        let res = match name {
            "sin" => x.sin(),
            "cos" => x.cos(),
            "tan" => x.tan(),
            "asin" => x.asin(),
            "acos" => x.acos(),
            "atan" => x.atan(),
            "sinh" => x.sinh(),
            "cosh" => x.cosh(),
            "tanh" => x.tanh(),
            "exp" => x.exp(),
            "log" => x.ln(),
            "log10" => x.log10(),
            "log2" => x.log2(),
            "sqrt" => x.sqrt(),
            "abs" => x.abs(),
            "sign" => {
                if x < 0.0 {
                    -1.0
                } else if x > 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            "floor" => x.floor(),
            "ceil" => x.ceil(),
            "round" => x.round(),
            "fix" => x.trunc(),
            _ => unreachable!(),
        };
        Ok(Value::Scalar(res))
    } else if TWO_ARG_FUNCS.contains(&name) {
        if args.len() != 2 {
            return err(format!("function '{name}' expects 2 arguments"), pos);
        }
        let (a, b) = (args[0], args[1]);
        let res = match name {
            "atan2" => a.atan2(b),
            "mod" => {
                if b == 0.0 {
                    a
                } else {
                    a - (a / b).floor() * b
                }
            }
            "rem" => {
                if b == 0.0 {
                    a
                } else {
                    a - (a / b).trunc() * b
                }
            }
            "min" => a.min(b),
            "max" => a.max(b),
            "hypot" => a.hypot(b),
            "power" => a.powf(b),
            _ => unreachable!(),
        };
        Ok(Value::Scalar(res))
    } else {
        err(format!("unknown function '{name}'"), pos)
    }
}

fn binop(a: &Value, b: &Value, op: char, pos: usize) -> Result<Value, ExprError> {
    match (a, b) {
        (Value::Scalar(x), Value::Scalar(y)) => Ok(Value::Scalar(match op {
            '+' => x + y,
            '-' => x - y,
            '*' => x * y,
            '/' => x / y,
            _ => x.powf(*y),
        })),
        _ => err("matrix arithmetic is not supported", pos),
    }
}

fn has_nan(v: &Value) -> bool {
    match v {
        Value::Scalar(x) => x.is_nan(),
        Value::Matrix(m) => m.data.iter().any(|x| x.is_nan()),
    }
}

/// Identifiers an expression refers to (variables), excluding function names and built-in constants.
pub fn identifiers(src: &str) -> Result<std::collections::BTreeSet<String>, ExprError> {
    let toks = lex(src)?;
    let mut set = std::collections::BTreeSet::new();
    for i in 0..toks.len() {
        if let Tok::Ident(ref name) = toks[i].tok {
            // Check if next token is LParen (function call)
            if i + 1 < toks.len() && toks[i + 1].tok == Tok::LParen {
                continue;
            }
            // Check if built-in constant
            match name.as_str() {
                "pi" | "inf" | "Inf" | "eps" => continue,
                _ => {
                    set.insert(name.clone());
                }
            }
        }
    }
    Ok(set)
}

/// Evaluates a parameter expression (no variables).
pub fn parse_value(src: &str) -> Result<Value, ExprError> {
    parse_value_in(src, &Workspace::new())
}

/// Evaluates a parameter expression; identifiers are looked up in `ws` first (#94).
pub fn parse_value_in(src: &str, ws: &Workspace) -> Result<Value, ExprError> {
    let toks = lex(src)?;
    if toks.len() == 1 {
        return err("empty expression", 0);
    }
    let mut p = Parser {
        toks,
        ws,
        i: 0,
        bracket_depth: 0,
        paren_depth: 0,
        depth: 0,
    };
    let v = p.parse_expr()?;
    if p.peek().tok != Tok::End {
        return err("unexpected token", p.peek().pos);
    }
    if has_nan(&v) {
        return err("expression evaluates to NaN", 0);
    }
    Ok(v)
}

#[cfg(test)]
#[allow(clippy::approx_constant)] // expected values are written out on purpose
mod tests {
    #[test]
    fn workspace_lookup() {
        let mut ws = crate::workspace::Workspace::new();
        ws.insert("Kp", Value::Scalar(2.0));
        assert_eq!(parse_value_in("Kp*3", &ws), Ok(Value::Scalar(6.0)));
        assert!(parse_value("Kp").is_err());
        // variables shadow built-in constants only if defined (pi is reserved, so never).
        assert_eq!(
            parse_value_in("pi", &ws),
            Ok(Value::Scalar(std::f64::consts::PI))
        );
    }

    use super::*;
    fn m(rows: usize, cols: usize, d: &[f64]) -> Value {
        Value::Matrix(Matrix {
            rows,
            cols,
            data: d.to_vec(),
        })
    }
    #[test]
    fn table() {
        let cases: Vec<(&str, Value)> = vec![
            ("3", Value::Scalar(3.0)),
            ("  -1.5e-3 ", Value::Scalar(-0.0015)),
            (".5", Value::Scalar(0.5)),
            ("1.", Value::Scalar(1.0)),
            ("2*pi", Value::Scalar(6.283185307179586)),
            ("1/(2*pi)", Value::Scalar(0.15915494309189535)),
            ("2^3^2", Value::Scalar(512.0)),
            ("-2^2", Value::Scalar(-4.0)),
            ("1 - 2", Value::Scalar(-1.0)),
            ("inf", Value::Scalar(f64::INFINITY)),
            ("Inf", Value::Scalar(f64::INFINITY)),
            ("-inf", Value::Scalar(f64::NEG_INFINITY)),
            ("1/0", Value::Scalar(f64::INFINITY)),
            ("[1 2 1]", m(1, 3, &[1.0, 2.0, 1.0])),
            ("[1, 2, 1]", m(1, 3, &[1.0, 2.0, 1.0])),
            ("[1 -2]", m(1, 2, &[1.0, -2.0])),
            ("[1 - 2]", m(1, 1, &[-1.0])),
            ("[1 -2+3]", m(1, 2, &[1.0, 1.0])),
            ("[1 0; 0 1]", m(2, 2, &[1.0, 0.0, 0.0, 1.0])),
            ("[1;2;3]", m(3, 1, &[1.0, 2.0, 3.0])),
            ("[2*pi (1+1)]", m(1, 2, &[6.283185307179586, 2.0])),
            ("[]", m(0, 0, &[])),
            ("[5]", m(1, 1, &[5.0])),
            ("sin(pi/2)", Value::Scalar(1.0)),
            ("atan2(1, 1)", Value::Scalar(std::f64::consts::FRAC_PI_4)),
            ("mod(-1, 3)", Value::Scalar(2.0)),
            ("rem(-1, 3)", Value::Scalar(-1.0)),
            ("mod(5, 0)", Value::Scalar(5.0)),
            ("max(2, 3) + min(2, 3)", Value::Scalar(5.0)),
            ("round(2.5)", Value::Scalar(3.0)),
            ("round(-2.5)", Value::Scalar(-3.0)),
            ("fix(-2.7)", Value::Scalar(-2.0)),
            ("sign(-3)", Value::Scalar(-1.0)),
            ("2 .* 3", Value::Scalar(6.0)),
            ("8 ./ 2", Value::Scalar(4.0)),
            ("2 .^ 3", Value::Scalar(8.0)),
            ("[1 2 3]'", m(3, 1, &[1.0, 2.0, 3.0])),
            ("[1 2; 3 4]'", m(2, 2, &[1.0, 3.0, 2.0, 4.0])),
            ("sqrt(2)/2", Value::Scalar(std::f64::consts::FRAC_1_SQRT_2)),
            ("[sin(0) cos(0)]", m(1, 2, &[0.0, 1.0])),
        ];
        for (src, want) in cases {
            assert_eq!(parse_value(src), Ok(want), "input {src:?}");
        }

        let mut ws = Workspace::new();
        ws.insert("az.Umax", Value::Scalar(1.5));
        assert_eq!(parse_value_in("az.Umax * 2", &ws), Ok(Value::Scalar(3.0)));
    }
    #[test]
    fn identifiers_extraction() {
        let idents = identifiers("Kp*sin(t0) + pi + az.U").unwrap();
        let expected: std::collections::BTreeSet<String> =
            ["Kp", "t0", "az.U"].into_iter().map(String::from).collect();
        assert_eq!(idents, expected);
    }
    #[test]
    fn nesting_is_limited() {
        // Deep but legal.
        let ok = format!("{}1{}", "(".repeat(30), ")".repeat(30));
        assert_eq!(parse_value(&ok), Ok(Value::Scalar(1.0)));
        // Hostile inputs must fail with an error, not overflow the stack.
        for src in [
            format!("{}1", "(".repeat(100_000)),
            format!("{}1", "-".repeat(100_000)),
            format!("{}1", "2^".repeat(100_000)),
            format!("{}1", "sin(".repeat(100_000)),
        ] {
            let e = parse_value(&src).unwrap_err();
            assert_eq!(e.message, "expression is nested too deeply");
        }
    }
    #[test]
    fn errors() {
        let cases = [
            ("", "empty expression", Some(0)),
            ("K", "unknown identifier 'K'", Some(0)),
            ("NaN", "unknown identifier 'NaN'", Some(0)),
            ("1 +", "unexpected", None),
            ("(1", "expected ')'", None),
            ("[1 2; 3]", "rows have different lengths", None),
            ("[[1]]", "nested matrices are not supported", None),
            ("[1 2]*2", "matrix arithmetic is not supported", None),
            ("0/0", "NaN", None),
            ("inf-inf", "NaN", None),
            ("1e", "invalid number", Some(0)),
            ("3 4", "unexpected token", Some(2)),
            ("2 $", "unexpected character '$'", Some(2)),
            ("foo(1)", "unknown function 'foo'", Some(0)),
            ("sin(1, 2)", "function 'sin' expects 1 argument", Some(0)),
            (
                "sin([1 2])",
                "function 'sin' expects scalar arguments",
                Some(4),
            ),
            ("sqrt(-1)", "expression evaluates to NaN", None),
            ("a.", "unexpected character", None),
        ];
        for (src, msg, pos) in cases {
            let e = parse_value(src).unwrap_err();
            assert!(e.message.contains(msg), "{src:?}: {e:?}");
            if let Some(p) = pos {
                assert_eq!(e.pos, p, "{src:?}");
            }
        }
        for src in ["[", "]", "(((", "-"] {
            assert!(parse_value(src).is_err(), "{src}");
        }

        let mut ws = Workspace::new();
        ws.insert("Kp", Value::Scalar(2.0));
        let e = parse_value_in("Kp(1)", &ws).unwrap_err();
        assert!(e.message.contains("indexing is not supported"), "{e:?}");
    }
    #[test]
    fn value_methods() {
        assert_eq!(Value::Scalar(2.0).as_scalar(), Some(2.0));
        assert_eq!(m(1, 1, &[5.0]).as_scalar(), Some(5.0));
        assert_eq!(m(1, 3, &[1.0, 2.0, 3.0]).as_scalar(), None);
        assert_eq!(
            m(3, 1, &[1.0, 2.0, 3.0]).as_vector(),
            Some(vec![1.0, 2.0, 3.0])
        );
        assert_eq!(m(2, 2, &[1.0; 4]).as_vector(), None);
        assert_eq!(
            Value::Scalar(4.0).as_matrix(),
            Matrix {
                rows: 1,
                cols: 1,
                data: vec![4.0]
            }
        );
    }
}
