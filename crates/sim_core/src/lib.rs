//! sim_core: model data, geometry, parameter expressions, block library,
//! validation, editing commands and clipboard support.

pub mod clipboard;
pub mod diag;
pub mod edit;
pub mod expr;
pub mod geom;
pub mod ids;
pub mod json;
pub mod library;
pub mod model;
pub mod path;
pub mod subsystem;
pub mod validate;
pub mod workspace;

/// Returns the crate name. Used only by the workspace smoke test.
pub fn crate_name() -> &'static str {
    "sim_core"
}
