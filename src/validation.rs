//! Parameter validation against prompt-spec.json ranges
//!
//! Validates AI-estimated values fall within defined ranges.

use crate::spec::{SPEC, Range, HeightRange};

/// A validation error with the parameter name and details
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} ({})", self.field, self.value, self.message)
    }
}

/// Parameters to validate
#[derive(Debug, Clone)]
pub struct EstimationParams {
    pub upper_area: Option<f64>,
    pub height: Option<f64>,
    pub slope: Option<f64>,
    pub fill_ratio_l: Option<f64>,
    pub fill_ratio_w: Option<f64>,
    pub fill_ratio_z: Option<f64>,
    pub packing_density: Option<f64>,
}

/// Validate all provided parameters against spec ranges.
/// Returns a list of validation errors (empty = all valid).
pub fn validate_params(params: &EstimationParams) -> Vec<ValidationError> {
    let ranges = &SPEC.ranges;
    let mut errors = Vec::new();

    if let Some(v) = params.upper_area {
        check_range("upperArea", v, &ranges.upper_area, &mut errors);
    }
    if let Some(v) = params.height {
        check_height_range("height", v, &ranges.height, &mut errors);
    }
    if let Some(v) = params.slope {
        check_range("slope", v, &ranges.slope, &mut errors);
    }
    if let Some(v) = params.fill_ratio_l {
        check_range("fillRatioL", v, &ranges.fill_ratio_l, &mut errors);
    }
    if let Some(v) = params.fill_ratio_w {
        check_range("fillRatioW", v, &ranges.fill_ratio_w, &mut errors);
    }
    if let Some(v) = params.fill_ratio_z {
        check_range("fillRatioZ", v, &ranges.fill_ratio_z, &mut errors);
    }
    if let Some(v) = params.packing_density {
        check_range("packingDensity", v, &ranges.packing_density, &mut errors);
    }

    errors
}

fn check_range(field: &str, value: f64, range: &Range, errors: &mut Vec<ValidationError>) {
    if value < range.min || value > range.max {
        errors.push(ValidationError {
            field: field.to_string(),
            value,
            min: range.min,
            max: range.max,
            message: format!("範囲外: {:.2}~{:.2}", range.min, range.max),
        });
    }
}

fn check_height_range(field: &str, value: f64, range: &HeightRange, errors: &mut Vec<ValidationError>) {
    if value < range.min || value > range.max {
        errors.push(ValidationError {
            field: field.to_string(),
            value,
            min: range.min,
            max: range.max,
            message: format!("範囲外: {:.2}~{:.2}", range.min, range.max),
        });
    }
}

/// Clamp a value to the specified range
pub fn clamp_to_range(value: f64, min: f64, max: f64) -> f64 {
    value.clamp(min, max)
}

/// Clamp all parameters to their valid ranges, returning the clamped values
pub fn clamp_params(params: &EstimationParams) -> EstimationParams {
    let r = &SPEC.ranges;
    EstimationParams {
        upper_area: params.upper_area.map(|v| v.clamp(r.upper_area.min, r.upper_area.max)),
        height: params.height.map(|v| v.clamp(r.height.min, r.height.max)),
        slope: params.slope.map(|v| v.clamp(r.slope.min, r.slope.max)),
        fill_ratio_l: params.fill_ratio_l.map(|v| v.clamp(r.fill_ratio_l.min, r.fill_ratio_l.max)),
        fill_ratio_w: params.fill_ratio_w.map(|v| v.clamp(r.fill_ratio_w.min, r.fill_ratio_w.max)),
        fill_ratio_z: params.fill_ratio_z.map(|v| v.clamp(r.fill_ratio_z.min, r.fill_ratio_z.max)),
        packing_density: params.packing_density.map(|v| v.clamp(r.packing_density.min, r.packing_density.max)),
    }
}

/// WASM-friendly validation (takes JSON string, returns JSON error array)
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "validateParams")]
pub fn validate_params_wasm(json: &str) -> String {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct WasmParams {
        upper_area: Option<f64>,
        height: Option<f64>,
        slope: Option<f64>,
        fill_ratio_l: Option<f64>,
        fill_ratio_w: Option<f64>,
        fill_ratio_z: Option<f64>,
        packing_density: Option<f64>,
    }

    let parsed: Result<WasmParams, _> = serde_json::from_str(json);
    match parsed {
        Ok(p) => {
            let params = EstimationParams {
                upper_area: p.upper_area,
                height: p.height,
                slope: p.slope,
                fill_ratio_l: p.fill_ratio_l,
                fill_ratio_w: p.fill_ratio_w,
                fill_ratio_z: p.fill_ratio_z,
                packing_density: p.packing_density,
            };
            let errors = validate_params(&params);
            let json_errors: Vec<serde_json::Value> = errors.iter().map(|e| {
                serde_json::json!({
                    "field": e.field,
                    "value": e.value,
                    "min": e.min,
                    "max": e.max,
                    "message": e.message,
                })
            }).collect();
            serde_json::to_string(&json_errors).unwrap_or_else(|_| "[]".to_string())
        }
        Err(e) => format!("[{{\"field\":\"parse\",\"message\":\"{}\"}}]", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_params() -> EstimationParams {
        EstimationParams {
            upper_area: Some(0.4),
            height: Some(0.45),
            slope: Some(0.1),
            fill_ratio_l: Some(0.9),
            fill_ratio_w: Some(0.9),
            fill_ratio_z: Some(0.85),
            packing_density: Some(0.7),
        }
    }

    #[test]
    fn test_valid_params_no_errors() {
        let errors = validate_params(&valid_params());
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_out_of_range_height() {
        let mut params = valid_params();
        params.height = Some(1.5); // max is 0.8
        let errors = validate_params(&params);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, "height");
    }

    #[test]
    fn test_out_of_range_packing_density() {
        let mut params = valid_params();
        params.packing_density = Some(0.3); // min is 0.5
        let errors = validate_params(&params);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, "packingDensity");
    }

    #[test]
    fn test_multiple_errors() {
        let mut params = valid_params();
        params.height = Some(-0.1);  // below min
        params.slope = Some(0.5);     // above max 0.3
        let errors = validate_params(&params);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_none_params_skip_validation() {
        let params = EstimationParams {
            upper_area: None,
            height: None,
            slope: None,
            fill_ratio_l: None,
            fill_ratio_w: None,
            fill_ratio_z: None,
            packing_density: None,
        };
        let errors = validate_params(&params);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_clamp_params() {
        let params = EstimationParams {
            upper_area: Some(0.1),      // below min 0.2
            height: Some(1.0),          // above max 0.8
            slope: Some(0.5),           // above max 0.3
            fill_ratio_l: Some(-0.1),   // below min 0.0
            fill_ratio_w: Some(1.5),    // above max 0.9
            fill_ratio_z: Some(0.5),    // below min 0.7
            packing_density: Some(0.3), // below min 0.5
        };
        let clamped = clamp_params(&params);
        assert!((clamped.upper_area.unwrap() - 0.2).abs() < f64::EPSILON);
        assert!((clamped.height.unwrap() - 0.8).abs() < f64::EPSILON);
        assert!((clamped.slope.unwrap() - 0.3).abs() < f64::EPSILON);
        assert!((clamped.fill_ratio_l.unwrap() - 0.0).abs() < f64::EPSILON);
        assert!((clamped.fill_ratio_w.unwrap() - 0.9).abs() < f64::EPSILON);
        assert!((clamped.fill_ratio_z.unwrap() - 0.75).abs() < f64::EPSILON);
        assert!((clamped.packing_density.unwrap() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_boundary_values_valid() {
        let params = EstimationParams {
            upper_area: Some(0.2),       // exact min
            height: Some(0.8),           // exact max
            slope: Some(0.0),            // exact min
            fill_ratio_l: Some(0.9),     // exact max
            fill_ratio_w: Some(0.0),     // exact min
            fill_ratio_z: Some(0.75),    // exact min
            packing_density: Some(0.9),  // exact max
        };
        let errors = validate_params(&params);
        assert!(errors.is_empty(), "Boundary values should be valid: {:?}", errors);
    }
}
