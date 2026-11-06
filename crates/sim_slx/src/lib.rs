//! sim_slx: SLX (Simulink OPC package) reading and writing.
//!
//! Layers: `opc` (ZIP/OPC, lossless) → `xml` (lossless XML tree) → `map` (SLX ⇔ Model, best effort).
//! The SLX internal format is not public; `map` is based on observed structure and is
//! **not verified against Simulink R2026b** (see issue M5-01).
pub mod error;
pub mod map;
pub mod opc;
pub mod xml;

pub use error::SlxError;
pub use map::{
    read_slx, read_slx_bytes, write_slx, write_slx_bytes, ImportReport, PreservedData, SlxImport,
};

/// Returns the crate name.
pub fn crate_name() -> &'static str {
    "sim_slx"
}
