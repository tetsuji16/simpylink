//! sim_codegen_py: SimIr → standalone Python (generator format 1, DES-04).

pub mod emit;
pub mod generate;
pub mod runtime;

pub use generate::generate;

/// Options for code generation.
#[derive(Debug, Clone, PartialEq)]
pub struct CodegenOptions {
    pub generator_version: String,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        CodegenOptions {
            generator_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Generated file.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedCode {
    pub file_name: String,
    pub source: String,
}

/// Generation failure.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CodegenError {
    #[error("unsupported block '{name}' (type {block_type}) in code generation")]
    UnsupportedBlock { name: String, block_type: String },
    #[error("invalid number {0} (NaN cannot be generated)")]
    InvalidNumber(String),
}

/// Returns the crate name. Used only by the workspace smoke test.
pub fn crate_name() -> &'static str {
    "sim_codegen_py"
}
