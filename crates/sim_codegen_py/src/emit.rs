//! Small helpers to write deterministic Python source.
use crate::CodegenError;
use sim_core::expr::Matrix;

/// Python float literal (Rust `{:?}` formatting; inf → math.inf).
pub fn py_float(v: f64) -> Result<String, CodegenError> {
    if v.is_nan() {
        return Err(CodegenError::InvalidNumber("NaN".into()));
    }
    if v == f64::INFINITY {
        return Ok("math.inf".into());
    }
    if v == f64::NEG_INFINITY {
        return Ok("-math.inf".into());
    }
    Ok(format!("{v:?}"))
}

/// Double-quoted Python string literal.
pub fn py_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            _ => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Makes `s` safe to embed in a one-line Python `#` comment.
///
/// Control characters and the Unicode line/paragraph separators are replaced by a
/// space; otherwise a block name containing a newline would end the comment and the
/// rest of the name would run as Python code.
pub fn py_comment(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() || c == '\u{2028}' || c == '\u{2029}' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// "np.array([a, b])".
pub fn py_vector(v: &[f64]) -> Result<String, CodegenError> {
    let items: Result<Vec<String>, _> = v.iter().map(|x| py_float(*x)).collect();
    Ok(format!("np.array([{}])", items?.join(", ")))
}

/// "np.array([[a, b], [c, d]])".
pub fn py_matrix(m: &Matrix) -> Result<String, CodegenError> {
    let mut rows = vec![];
    for r in 0..m.rows {
        let items: Result<Vec<String>, _> = (0..m.cols).map(|c| py_float(m.get(r, c))).collect();
        rows.push(format!("[{}]", items?.join(", ")));
    }
    Ok(format!("np.array([{}])", rows.join(", ")))
}

/// Line buffer with indentation.
#[derive(Debug, Default)]
pub struct PyWriter {
    buf: String,
    indent: usize,
}

impl PyWriter {
    /// New writer.
    pub fn new() -> Self {
        Self::default()
    }
    /// Writes one line (empty text → bare newline).
    pub fn line(&mut self, text: &str) {
        if !text.is_empty() {
            for _ in 0..self.indent {
                self.buf.push_str("    ");
            }
            self.buf.push_str(text);
        }
        self.buf.push('\n');
    }
    /// Empty line.
    pub fn blank(&mut self) {
        self.buf.push('\n');
    }
    /// Increase indentation.
    pub fn indent(&mut self) {
        self.indent += 1;
    }
    /// Decrease indentation (not below zero).
    pub fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }
    /// Final text.
    pub fn finish(self) -> String {
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comments_stay_on_one_line() {
        assert_eq!(py_comment("Gain 1"), "Gain 1");
        assert_eq!(py_comment("a\nimport os\r\tb"), "a import os  b");
        assert_eq!(py_comment("a\u{2028}b\u{2029}c\u{85}d"), "a b c d");
        assert_eq!(py_comment("ゲイン"), "ゲイン");
    }
    #[test]
    fn floats() {
        for (v, s) in [
            (3.0, "3.0"),
            (0.001, "0.001"),
            (1e-6, "1e-6"),
            (2.5, "2.5"),
            (-0.5, "-0.5"),
            (1e16, "1e16"),
            (0.1 + 0.2, "0.30000000000000004"),
            (-0.0, "-0.0"),
        ] {
            assert_eq!(py_float(v).unwrap(), s);
        }
        assert_eq!(py_float(f64::INFINITY).unwrap(), "math.inf");
        assert_eq!(py_float(f64::NEG_INFINITY).unwrap(), "-math.inf");
        assert!(py_float(f64::NAN).is_err());
        assert_eq!(py_str("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        let mut w = PyWriter::new();
        w.line("def f():");
        w.indent();
        w.line("return 1");
        w.dedent();
        w.dedent();
        w.line("");
        w.line("x = 2");
        assert_eq!(w.finish(), "def f():\n    return 1\n\nx = 2\n");
    }
}
