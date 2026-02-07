//! Tonnage calculation from geometric parameters
//!
//! Formula matches prompt-spec.json exactly:
//!   upperAreaM2 = fillRatioW × bedAreaM2
//!   effectiveHeight = max(height − slope/2, 0)
//!   volume = (upperAreaM2 + bedAreaM2) / 2 × effectiveHeight
//!   tonnage = volume × density × fillRatioZ × packingDensity

use crate::spec::{get_material_density, get_truck_bed_area, default_bed_area};

/// Input parameters for tonnage calculation
#[derive(Debug, Clone)]
pub struct CoreParams {
    pub fill_ratio_w: f64,
    pub height: f64,
    pub slope: f64,
    pub fill_ratio_z: f64,
    pub packing_density: f64,
    pub material_type: String,
}

/// Calculation result
#[derive(Debug, Clone)]
pub struct TonnageResult {
    /// Effective volume in m³
    pub volume: f64,
    /// Estimated tonnage
    pub tonnage: f64,
}

/// Calculate tonnage from geometric parameters
///
/// # Arguments
/// * `params` - Core geometric parameters from AI estimation
/// * `truck_class` - Optional truck class (e.g., "4t", "10t"). If None, uses default bed area.
pub fn calculate_tonnage(params: &CoreParams, truck_class: Option<&str>) -> TonnageResult {
    let bed_area = truck_class
        .map(|cls| get_truck_bed_area(cls))
        .unwrap_or_else(default_bed_area);

    let upper_area_m2 = params.fill_ratio_w * bed_area;
    let effective_height = (params.height - params.slope / 2.0).max(0.0);
    let volume = (upper_area_m2 + bed_area) / 2.0 * effective_height;

    let density = get_material_density(&params.material_type);
    let tonnage = volume * density * params.fill_ratio_z * params.packing_density;

    TonnageResult {
        volume: (volume * 1000.0).round() / 1000.0,  // 3 decimals
        tonnage: (tonnage * 100.0).round() / 100.0,   // 2 decimals
    }
}

/// WASM-friendly version (takes individual f64 parameters, returns JSON string)
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "calculateTonnage")]
pub fn calculate_tonnage_wasm(
    fill_ratio_w: f64,
    height: f64,
    slope: f64,
    fill_ratio_z: f64,
    packing_density: f64,
    material_type: &str,
    truck_class: Option<String>,
) -> String {
    let params = CoreParams {
        fill_ratio_w,
        height,
        slope,
        fill_ratio_z,
        packing_density,
        material_type: material_type.to_string(),
    };
    let result = calculate_tonnage(&params, truck_class.as_deref());
    serde_json::json!({
        "volume": result.volume,
        "tonnage": result.tonnage,
    }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_params() -> CoreParams {
        CoreParams {
            fill_ratio_w: 1.0,
            height: 0.40,
            slope: 0.0,
            fill_ratio_z: 0.85,
            packing_density: 0.80,
            material_type: "As殻".to_string(),
        }
    }

    #[test]
    fn test_calculate_basic() {
        let result = calculate_tonnage(&default_params(), Some("4t"));
        assert!(result.volume > 0.0);
        assert!(result.tonnage > 0.0);
    }

    #[test]
    fn test_zero_height_gives_zero() {
        let mut params = default_params();
        params.height = 0.0;
        let result = calculate_tonnage(&params, Some("4t"));
        assert!((result.volume).abs() < f64::EPSILON);
        assert!((result.tonnage).abs() < f64::EPSILON);
    }

    #[test]
    fn test_slope_reduces_effective_height() {
        let params_no_slope = default_params();
        let mut params_with_slope = default_params();
        params_with_slope.slope = 0.2;

        let result1 = calculate_tonnage(&params_no_slope, Some("4t"));
        let result2 = calculate_tonnage(&params_with_slope, Some("4t"));

        assert!(result2.tonnage < result1.tonnage);
    }

    #[test]
    fn test_slope_exceeding_height() {
        let mut params = default_params();
        params.height = 0.1;
        params.slope = 0.3; // slope/2 = 0.15 > height 0.1
        let result = calculate_tonnage(&params, Some("4t"));
        assert!((result.volume).abs() < f64::EPSILON);
    }

    #[test]
    fn test_material_density_affects_tonnage() {
        let mut params_as = default_params();
        params_as.material_type = "As殻".to_string(); // density 2.5

        let mut params_soil = default_params();
        params_soil.material_type = "土砂".to_string(); // density 1.8

        let result_as = calculate_tonnage(&params_as, Some("4t"));
        let result_soil = calculate_tonnage(&params_soil, Some("4t"));

        assert!(result_as.tonnage > result_soil.tonnage);
        // Same volume
        assert!((result_as.volume - result_soil.volume).abs() < 0.001);
    }

    #[test]
    fn test_default_bed_area_without_truck_class() {
        let result_none = calculate_tonnage(&default_params(), None);
        let result_4t = calculate_tonnage(&default_params(), Some("4t"));
        // 4t bed area = 3.4 * 2.06 = 7.004, default = 6.8
        // So they should be close but not identical
        assert!((result_none.tonnage - result_4t.tonnage).abs() < 1.0);
        assert!((result_none.tonnage - result_4t.tonnage).abs() > 0.01);
    }

    #[test]
    fn test_formula_matches_ts() {
        // Exact match test: same params as TS calculation.ts would produce
        let params = CoreParams {
            fill_ratio_w: 0.9,
            height: 0.45,
            slope: 0.1,
            fill_ratio_z: 0.85,
            packing_density: 0.75,
            material_type: "As殻".to_string(),
        };
        let result = calculate_tonnage(&params, Some("4t"));

        // Manual calculation:
        // bedArea = 3.4 * 2.06 = 7.004
        // upperAreaM2 = 0.9 * 7.004 = 6.3036
        // effectiveHeight = max(0.45 - 0.1/2, 0) = 0.40
        // volume = (6.3036 + 7.004) / 2 * 0.40 = 2.66152
        // tonnage = 2.66152 * 2.5 * 0.85 * 0.75 = 4.24117...
        assert!((result.volume - 2.662).abs() < 0.01);
        assert!((result.tonnage - 4.24).abs() < 0.02);
    }
}
