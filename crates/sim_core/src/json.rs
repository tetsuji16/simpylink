//! JSON format v1 load/save.
use crate::model::{Model, System, MODEL_FORMAT, MODEL_FORMAT_VERSION};
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
    #[error("unsupported format_version {0} (supported: 1)")]
    UnsupportedVersion(u32),
}

/// Pretty JSON + trailing newline. Deterministic.
pub fn to_json_string(model: &Model) -> String {
    // Model contains only string-keyed maps and numbers: serialization is infallible.
    let mut s = serde_json::to_string_pretty(model).expect("Model serialization is infallible");
    s.push('\n');
    s
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
    let v = value
        .get("format_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    if v != MODEL_FORMAT_VERSION {
        return Err(ModelIoError::UnsupportedVersion(v));
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
pub fn save_to_path(model: &Model, path: &Path) -> Result<(), ModelIoError> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, to_json_string(model))?;
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
