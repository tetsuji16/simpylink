//! Model variables ("model workspace") used by parameter expressions (#94, DES-02 §variables).
//!
//! A model stores its variables as `name → expression` strings (`Model::variables`).
//! [`Workspace::from_definitions`] evaluates them into numeric [`Value`]s; parameter
//! expressions are then evaluated with [`crate::expr::parse_value_in`].
//!
//! Skeleton status: name validation and lookup are implemented. Evaluating definitions
//! (dependency order, cycle detection, functions, struct fields) is specified in the
//! implementation issues linked from #94; until then non-empty definitions are rejected
//! with a clear error instead of being evaluated incorrectly.
use crate::expr::Value;
use std::collections::BTreeMap;

/// Names that cannot be used as variables (built-in constants of the expression language).
pub const RESERVED_NAMES: &[&str] = &["pi", "inf", "Inf", "eps", "NaN", "nan", "true", "false"];

/// Evaluated variables, looked up by name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Workspace {
    values: BTreeMap<String, Value>,
}

/// Why a variable definition could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("variable '{name}': {message}")]
pub struct VariableError {
    /// Variable name as written in `Model::variables`.
    pub name: String,
    /// Human-readable reason (English, lower-case first letter).
    pub message: String,
}

impl Workspace {
    /// Empty workspace.
    pub fn new() -> Workspace {
        Workspace::default()
    }

    /// Value of a variable. `name` may contain dots for struct fields (`az.Umax`).
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// Inserts or replaces a variable.
    pub fn insert(&mut self, name: &str, value: Value) {
        self.values.insert(name.to_string(), value);
    }

    /// Variable names in sorted order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    /// True if there are no variables.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Evaluates `Model::variables`.
    ///
    /// Contract (to be implemented, see #94 sub-issues): definitions may reference each
    /// other in any order; they are evaluated in dependency order; cycles and unknown
    /// identifiers are reported per variable; errors are sorted by name.
    pub fn from_definitions(
        defs: &BTreeMap<String, String>,
    ) -> Result<Workspace, Vec<VariableError>> {
        let mut errors: Vec<VariableError> = defs
            .keys()
            .filter(|n| !is_valid_variable_name(n))
            .map(|n| VariableError {
                name: n.clone(),
                message: "invalid variable name".into(),
            })
            .collect();
        if errors.is_empty() && !defs.is_empty() {
            errors = defs
                .keys()
                .map(|n| VariableError {
                    name: n.clone(),
                    message: "model variables are not supported yet".into(),
                })
                .collect();
        }
        if errors.is_empty() {
            Ok(Workspace::new())
        } else {
            Err(errors)
        }
    }
}

/// True for `[A-Za-z][A-Za-z0-9_]*` segments joined by `.` (struct fields), not reserved.
pub fn is_valid_variable_name(name: &str) -> bool {
    if RESERVED_NAMES.contains(&name) {
        return false;
    }
    !name.is_empty()
        && name.split('.').all(|seg| {
            let mut c = seg.chars();
            matches!(c.next(), Some(ch) if ch.is_ascii_alphabetic())
                && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(is_valid_variable_name("Kp"));
        assert!(is_valid_variable_name("az.Umax"));
        assert!(is_valid_variable_name("x_1"));
        assert!(!is_valid_variable_name("1x"));
        assert!(!is_valid_variable_name("_x"));
        assert!(!is_valid_variable_name("a..b"));
        assert!(!is_valid_variable_name("pi"));
        assert!(!is_valid_variable_name(""));
    }

    #[test]
    fn empty_definitions() {
        assert_eq!(
            Workspace::from_definitions(&BTreeMap::new()),
            Ok(Workspace::new())
        );
    }

    #[test]
    fn invalid_name_reported() {
        let defs = BTreeMap::from([("1x".to_string(), "1".to_string())]);
        let e = Workspace::from_definitions(&defs).unwrap_err();
        assert_eq!(e[0].name, "1x");
    }
}
