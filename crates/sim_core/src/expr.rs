//! Numeric parameter expressions (subset of MATLAB literal syntax).

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
                Tok::Ident(src[start..i].to_string())
            } else {
                i += 1;
                match c {
                    b'+' => Tok::Plus,
                    b'-' => Tok::Minus,
                    b'*' => Tok::Star,
                    b'/' => Tok::Slash,
                    b'^' => Tok::Caret,
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

struct Parser {
    toks: Vec<Token>,
    i: usize,
    bracket_depth: usize,
    paren_depth: usize,
}

impl Parser {
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
                Tok::Star | Tok::Slash => {
                    self.next();
                    let r = self.parse_unary()?;
                    v = binop(&v, &r, if t.tok == Tok::Star { '*' } else { '/' }, t.pos)?;
                }
                _ => break,
            }
        }
        Ok(v)
    }
    fn parse_unary(&mut self) -> Result<Value, ExprError> {
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
        if self.peek().tok == Tok::Caret && !self.is_element_boundary() {
            let t = self.next();
            let e = self.parse_unary()?;
            return binop(&base, &e, '^', t.pos);
        }
        Ok(base)
    }
    fn parse_primary(&mut self) -> Result<Value, ExprError> {
        let t = self.next();
        match t.tok {
            Tok::Num(v) => Ok(Value::Scalar(v)),
            Tok::Ident(ref s) => match s.as_str() {
                "pi" => Ok(Value::Scalar(std::f64::consts::PI)),
                "inf" | "Inf" => Ok(Value::Scalar(f64::INFINITY)),
                "eps" => Ok(Value::Scalar(f64::EPSILON)),
                _ => err(format!("unknown identifier '{s}'"), t.pos),
            },
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

/// Evaluates a parameter expression.
pub fn parse_value(src: &str) -> Result<Value, ExprError> {
    let toks = lex(src)?;
    if toks.len() == 1 {
        return err("empty expression", 0);
    }
    let mut p = Parser {
        toks,
        i: 0,
        bracket_depth: 0,
        paren_depth: 0,
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
        ];
        for (src, want) in cases {
            assert_eq!(parse_value(src), Ok(want), "input {src:?}");
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
