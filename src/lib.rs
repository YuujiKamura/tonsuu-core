//! tonsuu-core: トン数チェッカー コアライブラリ
//!
//! This crate provides the core calculation and validation
//! logic shared between the Rust CLI and TypeScript Web versions.
//!
//! Compiles to both native (rlib) and WebAssembly (cdylib via wasm-pack).

pub mod spec;
pub mod calculation;
pub mod prompt;
pub mod validation;

// Re-exports for convenience
pub use spec::{PromptSpec, TruckSpec, MaterialEntry, Range, HeightRange, Constants};
pub use calculation::{calculate_tonnage, height_from_geometry, TonnageResult, CoreParams};
#[allow(deprecated)]
pub use prompt::build_core_prompt;
pub use validation::{validate_params, ValidationError};
