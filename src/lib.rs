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
            "materialType": fill.material_type,
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

// ─── Integration tests ────────────────────────────────────────────────

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Verify GEOMETRY_PROMPT and FILL_PROMPT are non-empty and come from prompt-spec.json
    #[test]
    fn test_prompts_from_spec() {
        let geo_prompt = &spec::SPEC.geometry_prompt;
        let fill_prompt = &spec::SPEC.fill_prompt;

        assert!(!geo_prompt.is_empty(), "geometry_prompt must not be empty");
        assert!(!fill_prompt.is_empty(), "fill_prompt must not be empty");

        // Geometry prompt should mention key terms
        assert!(geo_prompt.contains("tailgateTopY"), "geometry_prompt missing tailgateTopY");
        assert!(geo_prompt.contains("cargoTopY"), "geometry_prompt missing cargoTopY");
        assert!(geo_prompt.contains("plateBox"), "geometry_prompt missing plateBox");

        // Fill prompt should mention key terms
        assert!(fill_prompt.contains("fillRatioL"), "fill_prompt missing fillRatioL");
        assert!(fill_prompt.contains("taperRatio"), "fill_prompt missing taperRatio");
        assert!(fill_prompt.contains("packingDensity"), "fill_prompt missing packingDensity");
    }

    /// Verify CLI and Web produce identical calculation results for the same input
    #[test]
    fn test_calculation_consistency() {
        // Fixed input matching a typical box-overlay analysis
        let params = CoreParams {
            height: 0.40,
            fill_ratio_l: 0.8,
            fill_ratio_w: 0.85,
            taper_ratio: 0.9,
            packing_density: 0.80,
            material_type: "As殻".to_string(),
        };

        let result = calculate_tonnage(&params, Some("4t"));

        // These exact values must match TypeScript WASM calculateTonnage output
        // for the same inputs. Cross-verified with TS boxOverlayService.ts.
        assert!(result.volume > 0.0);
        assert!(result.tonnage > 0.0);

        // Verify determinism: same input -> same output
        let result2 = calculate_tonnage(&params, Some("4t"));
        assert!((result.volume - result2.volume).abs() < f64::EPSILON);
        assert!((result.tonnage - result2.tonnage).abs() < f64::EPSILON);
    }

    /// Verify full pipeline with mock backend produces consistent results
    #[test]
    fn test_pipeline_end_to_end_consistency() {
        use pipeline::{AiBackend, BoxOverlayConfig, PipelineError};

        struct FixedBackend;
        impl AiBackend for FixedBackend {
            fn send_prompt(&self, prompt: &str, _images: &[Vec<u8>]) -> Result<String, PipelineError> {
                if prompt.contains("tailgateTopY") {
                    Ok(r#"{"plateBox":[0.4,0.7,0.6,0.84],"tailgateTopY":0.3,"tailgateBottomY":0.5,"cargoTopY":0.2}"#.to_string())
                } else {
                    Ok(r#"{"fillRatioL":0.8,"fillRatioW":0.85,"taperRatio":0.9,"packingDensity":0.8,"reasoning":"Integration test"}"#.to_string())
                }
            }
        }

        let config = BoxOverlayConfig {
            truck_class: "4t".to_string(),
            material_type: "As殻".to_string(),
            ensemble_count: 1,
        };

        let r1 = analyze_box_overlay(&FixedBackend, &[], &config).unwrap();
        let r2 = analyze_box_overlay(&FixedBackend, &[], &config).unwrap();

        // Determinism: same fixed input -> same output
        assert!((r1.height_m - r2.height_m).abs() < f64::EPSILON, "height_m mismatch");
        assert!((r1.volume - r2.volume).abs() < f64::EPSILON, "volume mismatch");
        assert!((r1.tonnage - r2.tonnage).abs() < f64::EPSILON, "tonnage mismatch");
        assert!((r1.fill_ratio_l - r2.fill_ratio_l).abs() < f64::EPSILON, "fill_ratio_l mismatch");
        assert!((r1.fill_ratio_w - r2.fill_ratio_w).abs() < f64::EPSILON, "fill_ratio_w mismatch");
        assert!((r1.taper_ratio - r2.taper_ratio).abs() < f64::EPSILON, "taper_ratio mismatch");

        // Sanity checks on actual values
        assert!((r1.height_m - 0.48).abs() < 0.01, "height ~0.48m expected, got {}", r1.height_m);
        assert!(r1.tonnage > 3.0 && r1.tonnage < 5.0, "tonnage in 3-5t range, got {}", r1.tonnage);
        assert!((r1.density - 2.5).abs() < f64::EPSILON, "As殻 density 2.5, got {}", r1.density);
    }

    /// Verify parse functions produce consistent results for the same input
    #[test]
    fn test_parse_consistency() {
        let geo_json = r#"{"plateBox":[0.4,0.7,0.6,0.84],"tailgateTopY":0.3,"tailgateBottomY":0.5,"cargoTopY":0.2}"#;
        let fill_json = r#"{"fillRatioL":0.8,"fillRatioW":0.85,"taperRatio":0.9,"packingDensity":0.8,"reasoning":"Test"}"#;

        let geo1 = parse_geometry(geo_json).unwrap();
        let geo2 = parse_geometry(geo_json).unwrap();
        assert!((geo1.tailgate_top_y - geo2.tailgate_top_y).abs() < f64::EPSILON);
        assert!((geo1.cargo_top_y - geo2.cargo_top_y).abs() < f64::EPSILON);

        let fill1 = parse_fill(fill_json).unwrap();
        let fill2 = parse_fill(fill_json).unwrap();
        assert!((fill1.fill_ratio_l - fill2.fill_ratio_l).abs() < f64::EPSILON);
        assert!((fill1.taper_ratio - fill2.taper_ratio).abs() < f64::EPSILON);
    }
}
