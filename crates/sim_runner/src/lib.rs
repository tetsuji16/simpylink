//! sim_runner: runs generated Python scripts in a separate process (DES-04 §6).

pub mod process;
pub mod python;
pub mod results;
pub mod types;

pub use process::{run_blocking, RunHandle};
pub use python::{candidates, check_environment, discover_python, parse_version};
pub use results::{load_results, parse_csv, parse_meta, parse_number};
pub use types::*;

/// Returns the crate name. Used only by the workspace smoke test.
pub fn crate_name() -> &'static str {
    "sim_runner"
}
