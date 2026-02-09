//! Tonnage calculation from geometric parameters
//!
//! Box-overlay formula (v2.1):
//!   effectiveL = fillRatioL * taperRatio
//!   effectiveW = (BOTTOM_FILL + fillRatioW) / 2
//!   volume = bedL * bedW * height * effectiveL * effectiveW
//!   compressionFactor = 1.0 + 0.15 * (volume - 2.0)
//!   effectivePacking = clamp(packing * compressionFactor, 0.7, 0.95)
//!   tonnage = volume * density * effectivePacking

use crate::spec::{get_material_density, get_truck_spec, default_bed_area, SPEC};

/// Input parameters for box-overlay tonnage calculation
#[derive(Debug, Clone)]
pub struct CoreParams {
    pub height: f64,
    pub fill_ratio_l: f64,
    pub fill_ratio_w: f64,
    pub taper_ratio: f64,
    pub packing_density: f64,
    pub material_type: String,
}

/// Calculation result
#[derive(Debug, Clone)]
pub struct TonnageResult {
    /// Effective volume in m3
    pub volume: f64,
    /// Estimated tonnage
    pub tonnage: f64,
    /// Effective packing density after compression correction
    pub effective_packing: f64,
    /// Material density used
    pub density: f64,
}

/// Calculate tonnage using box-overlay formula
pub fn calculate_tonnage(params: &CoreParams, truck_class: Option<&str>) -> TonnageResult {
    let c = &SPEC.constants;

    let (bed_l, bed_w) = truck_class
        .and_then(|cls| get_truck_spec(cls))
        .map(|s| (s.bed_length, s.bed_width))
        .unwrap_or_else(|| {
            let area = default_bed_area();
            // Approximate: assume 4t proportions
            (3.4, area / 3.4)
        });

    let effective_l = params.fill_ratio_l * params.taper_ratio;
    let effective_w = (c.bottom_fill + params.fill_ratio_w) / 2.0;
    let volume = bed_l * bed_w * params.height * effective_l * effective_w;

    let compression_factor = 1.0 + c.compression_factor * (volume - c.compression_ref_volume);
    let effective_packing = (params.packing_density * compression_factor)
        .clamp(c.effective_packing_min, c.effective_packing_max);

    let density = get_material_density(&params.material_type);
    let tonnage = volume * density * effective_packing;

    TonnageResult {
        volume: round3(volume),
        tonnage: round2(tonnage),
        effective_packing: round3(effective_packing),
        density,
    }
}

/// Geometry-based height calculation from normalized image coordinates
///
/// Returns (height_m, scale_method)
/// - "tailgate": scaled from tailgate top/bottom distance
/// - "plate": scaled from license plate height (fallback)
/// - "none": no valid scale reference found
pub fn height_from_geometry(
    tg_top: f64,
    tg_bot: f64,
    cargo_top: f64,
    plate_box: Option<[f64; 4]>,
    bed_height: f64,
) -> (f64, &'static str) {
    let c = &SPEC.constants;

    let has_tailgate = tg_bot > 0.0 && tg_bot > tg_top;

    let plate_height_norm = plate_box
        .map(|pb| pb[3] - pb[1])
        .unwrap_or(0.0);
    let has_plate = plate_height_norm > c.plate_min_norm;

    if !has_plate && !has_tailgate {
        return (0.0, "none");
    }

    let (cargo_height_m, method) = if has_tailgate {
        let tg_height_norm = tg_bot - tg_top;
        let m_per_norm = bed_height / tg_height_norm;
        let h = (tg_bot - cargo_top) * m_per_norm;
        (h, "tailgate")
    } else {
        let m_per_norm = c.plate_height_m / plate_height_norm;
        let h = bed_height + (tg_top - cargo_top) * m_per_norm;
        (h, "plate")
    };

    (cargo_height_m.clamp(0.0, 0.8), method)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// WASM-friendly version
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "calculateTonnage")]
pub fn calculate_tonnage_wasm(
    height: f64,
    fill_ratio_l: f64,
    fill_ratio_w: f64,
    taper_ratio: f64,
    packing_density: f64,
    material_type: &str,
    truck_class: Option<String>,
) -> String {
    let params = CoreParams {
        height,
        fill_ratio_l,
        fill_ratio_w,
        taper_ratio,
        packing_density,
        material_type: material_type.to_string(),
    };
    let result = calculate_tonnage(&params, truck_class.as_deref());
    serde_json::json!({
        "volume": result.volume,
        "tonnage": result.tonnage,
        "effectivePacking": result.effective_packing,
        "density": result.density,
    }).to_string()
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "heightFromGeometry")]
pub fn height_from_geometry_wasm(
    tg_top: f64,
    tg_bot: f64,
    cargo_top: f64,
    plate_box_json: Option<String>,
    bed_height: f64,
) -> String {
    let plate_box: Option<[f64; 4]> = plate_box_json
        .and_then(|s| serde_json::from_str(&s).ok());
    let (height_m, method) = height_from_geometry(tg_top, tg_bot, cargo_top, plate_box, bed_height);
    serde_json::json!({
        "heightM": height_m,
        "scaleMethod": method,
    }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_params() -> CoreParams {
        CoreParams {
            height: 0.40,
            fill_ratio_l: 0.8,
            fill_ratio_w: 0.85,
            taper_ratio: 0.85,
            packing_density: 0.80,
            material_type: "As殻".to_string(),
        }
    }

    #[test]
    fn test_calculate_basic() {
        let result = calculate_tonnage(&default_params(), Some("4t"));
        assert!(result.volume > 0.0);
        assert!(result.tonnage > 0.0);
        assert!(result.effective_packing > 0.0);
    }

    #[test]
    fn test_zero_height_gives_zero() {
        let mut params = default_params();
        params.height = 0.0;
        let result = calculate_tonnage(&params, Some("4t"));
        assert!(result.volume.abs() < f64::EPSILON);
        assert!(result.tonnage.abs() < f64::EPSILON);
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
    fn test_formula_matches_ts() {
        // Match the TypeScript calculateBoxOverlay function exactly
        let params = CoreParams {
            height: 0.40,
            fill_ratio_l: 0.8,
            fill_ratio_w: 0.85,
            taper_ratio: 0.9,
            packing_density: 0.80,
            material_type: "As殻".to_string(),
        };
        let result = calculate_tonnage(&params, Some("4t"));

        // Manual calculation:
        // bedL=3.4, bedW=2.06
        // effectiveL = 0.8 * 0.9 = 0.72
        // effectiveW = (0.9 + 0.85) / 2 = 0.875
        // volume = 3.4 * 2.06 * 0.40 * 0.72 * 0.875 = 1.76411...
        // compressionFactor = 1.0 + 0.15 * (1.764 - 2.0) = 0.9646
        // effectivePacking = clamp(0.80 * 0.9646, 0.7, 0.95) = 0.77168
        // tonnage = 1.764 * 2.5 * 0.772 = 3.40...
        assert!((result.volume - 1.764).abs() < 0.01);
        assert!(result.tonnage > 3.0 && result.tonnage < 4.0);
    }

    #[test]
    fn test_compression_clamp() {
        // Very large volume should cap effective_packing at 0.95
        let params = CoreParams {
            height: 0.70,
            fill_ratio_l: 0.9,
            fill_ratio_w: 0.9,
            taper_ratio: 1.0,
            packing_density: 0.9,
            material_type: "As殻".to_string(),
        };
        let result = calculate_tonnage(&params, Some("10t"));
        assert!(result.effective_packing <= 0.95);
    }

    #[test]
    fn test_height_from_geometry_tailgate() {
        // tailgate top=0.3, bot=0.5, cargo_top=0.2, bed_height=0.32
        // tg_height_norm = 0.2, m_per_norm = 0.32/0.2 = 1.6
        // cargo_h = (0.5 - 0.2) * 1.6 = 0.48
        let (h, method) = height_from_geometry(0.3, 0.5, 0.2, None, 0.32);
        assert_eq!(method, "tailgate");
        assert!((h - 0.48).abs() < 0.01);
    }

    #[test]
    fn test_height_from_geometry_plate_fallback() {
        // tg_bot invalid (0), plate_box = [0.4, 0.7, 0.6, 0.84]
        // plate_h_norm = 0.84 - 0.7 = 0.14, m_per_norm = 0.22 / 0.14 = 1.571
        // cargo_h = 0.32 + (0.3 - 0.15) * 1.571 = 0.32 + 0.236 = 0.556
        let (h, method) = height_from_geometry(0.3, 0.0, 0.15, Some([0.4, 0.7, 0.6, 0.84]), 0.32);
        assert_eq!(method, "plate");
        assert!(h > 0.4 && h < 0.8);
    }

    #[test]
    fn test_height_from_geometry_no_reference() {
        let (h, method) = height_from_geometry(0.3, 0.0, 0.2, None, 0.32);
        assert_eq!(method, "none");
        assert!(h.abs() < f64::EPSILON);
    }

    #[test]
    fn test_height_clamped_to_08() {
        // Very high cargo should clamp to 0.8
        let (h, _) = height_from_geometry(0.5, 0.9, 0.0, None, 0.50);
        assert!(h <= 0.8);
    }
}
