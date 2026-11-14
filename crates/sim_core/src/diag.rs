//! Diagnostics shared by validation, compiler and GUI.
use crate::ids::{BlockId, LineId};
use crate::path::SystemPath;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Where a diagnostic applies.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Location {
    pub path: SystemPath,
    pub block: Option<BlockId>,
    pub line: Option<LineId>,
}

impl Location {
    /// Location of a block.
    pub fn block(path: &SystemPath, sid: BlockId) -> Self {
        Location {
            path: path.clone(),
            block: Some(sid),
            line: None,
        }
    }
    /// Location of a line.
    pub fn line(path: &SystemPath, id: LineId) -> Self {
        Location {
            path: path.clone(),
            block: None,
            line: Some(id),
        }
    }
}

/// One diagnostic message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub location: Location,
}

impl Diagnostic {
    /// Error.
    pub fn error(code: &str, msg: impl Into<String>, loc: Location) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code: code.into(),
            message: msg.into(),
            location: loc,
        }
    }
    /// Warning.
    pub fn warning(code: &str, msg: impl Into<String>, loc: Location) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code: code.into(),
            message: msg.into(),
            location: loc,
        }
    }
    /// Info.
    pub fn info(code: &str, msg: impl Into<String>, loc: Location) -> Self {
        Diagnostic {
            severity: Severity::Info,
            code: code.into(),
            message: msg.into(),
            location: loc,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        write!(f, "{sev}[{}]: {}", self.code, self.message)?;
        if let Some(b) = self.location.block {
            write!(f, " (block {b})")
        } else if let Some(l) = self.location.line {
            write!(f, " (line {l})")
        } else {
            Ok(())
        }
    }
}

/// True if any diagnostic is an error.
pub fn has_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}

/// Diagnostic codes.
pub mod codes {
    pub const E_DUPLICATE_SID: &str = "E_DUPLICATE_SID";
    pub const E_DUPLICATE_LINE_ID: &str = "E_DUPLICATE_LINE_ID";
    pub const E_DUPLICATE_BLOCK_NAME: &str = "E_DUPLICATE_BLOCK_NAME";
    pub const E_UNKNOWN_BLOCK_TYPE: &str = "E_UNKNOWN_BLOCK_TYPE";
    pub const E_INVALID_PARAM: &str = "E_INVALID_PARAM";
    pub const E_LINE_SRC_MISSING: &str = "E_LINE_SRC_MISSING";
    pub const E_LINE_DST_MISSING: &str = "E_LINE_DST_MISSING";
    pub const E_PORT_OUT_OF_RANGE: &str = "E_PORT_OUT_OF_RANGE";
    pub const E_INPUT_MULTIPLY_DRIVEN: &str = "E_INPUT_MULTIPLY_DRIVEN";
    pub const E_INVALID_CONFIG: &str = "E_INVALID_CONFIG";
    pub const W_UNCONNECTED_INPUT: &str = "W_UNCONNECTED_INPUT";
    pub const W_UNCONNECTED_OUTPUT: &str = "W_UNCONNECTED_OUTPUT";
    pub const W_DANGLING_LINE: &str = "W_DANGLING_LINE";
    pub const E_PARAM_EVAL: &str = "E_PARAM_EVAL";
    pub const E_PARAM_DOMAIN: &str = "E_PARAM_DOMAIN";
    pub const E_ALGEBRAIC_LOOP: &str = "E_ALGEBRAIC_LOOP";
    pub const E_SAMPLE_TIME: &str = "E_SAMPLE_TIME";
    pub const E_UNSUPPORTED_BLOCK: &str = "E_UNSUPPORTED_BLOCK";
    pub const E_UNSUPPORTED_FEATURE: &str = "E_UNSUPPORTED_FEATURE";
    /// Duplicate local or global Goto tag.
    pub const E_DUPLICATE_GOTO_TAG: &str = "E_DUPLICATE_GOTO_TAG";
    /// A From block has no visible matching Goto.
    pub const E_UNMATCHED_FROM: &str = "E_UNMATCHED_FROM";
    /// A model variable definition is invalid or cannot be evaluated (#94).
    pub const E_VARIABLE: &str = "E_VARIABLE";
    /// Signal widths do not match (vector signals, #92 / DES-07 §3).
    pub const E_DIMENSION: &str = "E_DIMENSION";
    /// A data-type parameter was ignored; simpylink computes in double (DES-07 §5).
    pub const W_DATA_TYPE_IGNORED: &str = "W_DATA_TYPE_IGNORED";
    /// An atomic subsystem was flattened like a virtual one (DES-07 §6).
    pub const W_ATOMIC_AS_VIRTUAL: &str = "W_ATOMIC_AS_VIRTUAL";
    /// A block, line or Goto tag name is empty or contains control characters (#198).
    pub const E_INVALID_NAME: &str = "E_INVALID_NAME";
    /// A block position or line point is NaN or ±Inf (#198).
    pub const E_NON_FINITE_GEOMETRY: &str = "E_NON_FINITE_GEOMETRY";
    /// Two Inports (or two Outports) of one system use the same `Port` number (#196).
    pub const E_DUPLICATE_PORT_NUMBER: &str = "E_DUPLICATE_PORT_NUMBER";
    /// Inport (or Outport) `Port` numbers are not `1..=count` without gaps (#196).
    pub const E_PORT_NUMBER_GAP: &str = "E_PORT_NUMBER_GAP";
}
