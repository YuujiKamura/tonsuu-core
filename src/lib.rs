//! tonsuu-core: トン数チェッカー コアライブラリ
//!
//! This crate provides the core calculation and validation
//! logic shared between the Rust CLI and TypeScript Web versions.
//!
//! Compiles to both native (rlib) and WebAssembly (cdylib via wasm-pack).

pub mod spec;
pub mod calculation;
pub mod parse;
pub mod pipeline;
pub mod prompt;
pub mod validation;

// Re-exports for convenience
pub use spec::{PromptSpec, TruckSpec, MaterialEntry, Range, HeightRange, Constants};
pub use calculation::{calculate_tonnage, height_from_geometry, TonnageResult, CoreParams};
pub use parse::{parse_geometry, parse_fill, GeometryResponse, FillResponse, ParseError};
pub use pipeline::{
    analyze_box_overlay, AiBackend, BoxOverlayConfig, BoxOverlayResult,
    PipelineError, GeometryRunLog, FillRunLog,
};
#[allow(deprecated)]
pub use prompt::build_core_prompt;
pub use validation::{validate_params, ValidationError};

// ─── WASM exports for prompt access and parsing ──────────────────────

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "getGeometryPrompt")]
pub fn get_geometry_prompt_wasm() -> String {
    spec::SPEC.geometry_prompt.clone()
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "getFillPrompt")]
pub fn get_fill_prompt_wasm() -> String {
    spec::SPEC.fill_prompt.clone()
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "parseGeometry")]
pub fn parse_geometry_wasm(text: &str) -> String {
    match parse::parse_geometry(text) {
        Ok(geo) => serde_json::json!({
            "ok": true,
            "plateBox": geo.plate_box,
            "tailgateTopY": geo.tailgate_top_y,
            "tailgateBottomY": geo.tailgate_bottom_y,
            "cargoTopY": geo.cargo_top_y,
        })
        .to_string(),
        Err(e) => serde_json::json!({
            "ok": false,
            "error": e.message,
        })
        .to_string(),
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "parseFill")]
pub fn parse_fill_wasm(text: &str) -> String {
    match parse::parse_fill(text) {
        Ok(fill) => serde_json::json!({
            "ok": true,
            "fillRatioL": fill.fill_ratio_l,
            "fillRatioW": fill.fill_ratio_w,
            "taperRatio": fill.taper_ratio,
            "packingDensity": fill.packing_density,
            "reasoning": fill.reasoning,
        })
        .to_string(),
        Err(e) => serde_json::json!({
            "ok": false,
            "error": e.message,
        })
        .to_string(),
    }
}
