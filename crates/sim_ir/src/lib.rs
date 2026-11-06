//! sim_ir: compiles a `sim_core` model into a Simulation IR (DES-03).

pub mod compile;
pub mod graph;
pub mod lower;
pub mod naming;
pub mod realize;
pub mod sample_time;
pub mod sort;
pub mod types;

pub use compile::{compile, solver_method, CompileFailure, CompiledModel};
pub use types::*;

/// Returns the crate name. Used only by the workspace smoke test.
pub fn crate_name() -> &'static str {
    "sim_ir"
}
