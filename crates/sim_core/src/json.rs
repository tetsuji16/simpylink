//! JSON format v1 load/save.
use crate::model::{Model, System, MODEL_FORMAT, MODEL_FORMAT_VERSION, MODEL_FORMAT_VERSION_MAX};
use std::path::Path;

/// Errors while reading or writing model files.
#[derive(Debug, thiserror::Error)]
pub enum ModelIoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error at line {line}, column {column}: {message}")]
    Parse {
        line: usize,
        column: usize,
        message: String,
    },
    #[error("not a simpylink model (format = {0:?})")]
    WrongFormat(String),
    #[error("unsupported format_version {0} (supported: 1, 2)")]
    UnsupportedVersion(u32),
    /// A field of the JSON document is present but not usable (#198).
    #[error("{field}: {message}")]
    InvalidField { field: String, message: String },
}

/// Message for a `format_version` that is not an integer in the supported range.
fn version_message() -> String {
    format!("must be an integer between {MODEL_FORMAT_VERSION} and {MODEL_FORMAT_VERSION_MAX}")
}

/// Pretty JSON + trailing newline. Deterministic. `format_version` is written as
/// [`Model::required_format_version`], so models without v2 features stay v1.
pub fn to_json_string(model: &Model) -> String {
    match try_to_json_string(model) {
        Ok(s) => s,
        // Unreachable: `Model` holds string-keyed maps, enums and numbers, which
        // `serde_json` always serializes. Report the failure instead of returning text
        // that looks like a model; `save_to_path` propagates the error instead.
        Err(e) => {
            let mut s = serde_json::json!({ "serialization_error": e.to_string() }).to_string();
            s.push('\n');
            s
        }
    }
}

/// Pretty JSON + trailing newline, or the reason the model cannot be written.
///
/// The model is serialized directly (not through `serde_json::Value`, which would
/// reorder the keys) so the field order of the file stays stable.
pub fn try_to_json_string(model: &Model) -> Result<String, ModelIoError> {
    let mut m = model.clone();
    m.format_version = model.required_format_version();
    let mut s = serde_json::to_string_pretty(&m).map_err(|e| ModelIoError::Parse {
        line: 0,
        column: 0,
        message: e.to_string(),
    })?;
    s.push('\n');
    Ok(s)
}

fn sort_system(s: &mut System) {
    s.blocks.sort_by_key(|b| b.sid);
    s.lines.sort_by_key(|l| l.id);
    for b in &mut s.blocks {
        if let Some(c) = b.system.as_mut() {
            sort_system(c);
        }
    }
}

/// Parses and normalizes a model.
pub fn from_json_str(s: &str) -> Result<Model, ModelIoError> {
    let value: serde_json::Value = serde_json::from_str(s).map_err(|e| ModelIoError::Parse {
        line: e.line(),
        column: e.column(),
        message: e.to_string(),
    })?;
    match value.get("format") {
        Some(serde_json::Value::String(f)) if f == MODEL_FORMAT => {}
        Some(serde_json::Value::String(f)) => return Err(ModelIoError::WrongFormat(f.clone())),
        Some(other) => return Err(ModelIoError::WrongFormat(other.to_string())),
        None => return Err(ModelIoError::WrongFormat(String::new())),
    }
    // `format_version` must be an integer that fits in `u32`: reading it with
    // `as_u64().unwrap_or(0) as u32` silently truncated 4294967297 to 1 (#198).
    let version = match value.get("format_version") {
        Some(serde_json::Value::Number(n)) => n
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| ModelIoError::InvalidField {
                field: "format_version".into(),
                message: version_message(),
            })?,
        _ => {
            return Err(ModelIoError::InvalidField {
                field: "format_version".into(),
                message: version_message(),
            })
        }
    };
    if !(MODEL_FORMAT_VERSION..=MODEL_FORMAT_VERSION_MAX).contains(&version) {
        return Err(ModelIoError::UnsupportedVersion(version));
    }
    let mut m: Model = serde_json::from_value(value).map_err(|e| ModelIoError::Parse {
        line: 0,
        column: 0,
        message: e.to_string(),
    })?;
    sort_system(&mut m.root);
    m.next_sid = m.next_sid.max(m.max_sid() + 1);
    m.next_line_id = m.next_line_id.max(m.max_line_id() + 1);
    Ok(m)
}

/// Atomic save (write temp file then rename).
///
/// A model whose geometry is not finite cannot be written: `serde_json` would store
/// NaN / ±Inf as `null`, which does not load again (#198).
pub fn save_to_path(model: &Model, path: &Path) -> Result<(), ModelIoError> {
    if crate::validate::has_non_finite_geometry(model) {
        return Err(ModelIoError::InvalidField {
            field: "geometry".into(),
            message: "block positions and line points must be finite".into(),
        });
    }
    let text = try_to_json_string(model)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ignored_cleanup = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Loads a model file.
pub fn load_from_path(path: &Path) -> Result<Model, ModelIoError> {
    let s = std::fs::read_to_string(path)?;
    from_json_str(&s)
}
