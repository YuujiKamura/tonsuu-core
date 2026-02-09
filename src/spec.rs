//! prompt-spec.json parser and typed accessors
//!
//! Embeds prompt-spec.json at compile time via `include_str!` and provides
//! the single source of truth for all domain constants (ranges, trucks, materials).
//!
//! Prompts are NOT embedded here — they are read at runtime by each consumer.

use std::collections::HashMap;
use std::sync::LazyLock;
use serde::Deserialize;

/// Raw JSON embedded at compile time
const SPEC_JSON: &str = include_str!("../prompt-spec.json");

/// Parsed prompt-spec.json (singleton)
pub static SPEC: LazyLock<PromptSpec> = LazyLock::new(|| {
    serde_json::from_str(SPEC_JSON).expect("Failed to parse embedded prompt-spec.json")
});

/// Top-level prompt specification (v2.1.0)
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PromptSpec {
    pub version: String,
    pub materials: HashMap<String, MaterialEntry>,
    pub truck_specs: HashMap<String, TruckSpec>,
    pub ranges: Ranges,
    pub constants: Constants,
    pub geometry_prompt: String,
    pub fill_prompt: String,
}

/// Parameter ranges for box-overlay strategy
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Ranges {
    pub height: HeightRange,
    pub fill_ratio_l: Range,
    pub fill_ratio_w: Range,
    pub taper_ratio: Range,
    pub packing_density: Range,
    /// Legacy: kept for multi-param strategy backward compat
    pub fill_ratio_z: Range,
}

/// Simple min/max range
#[derive(Debug, Deserialize, Clone)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

/// Height range with step and calibration landmarks
#[derive(Debug, Deserialize, Clone)]
pub struct HeightRange {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub calibration: HeightCalibration,
}

/// Height calibration landmarks
#[derive(Debug, Deserialize, Clone)]
pub struct HeightCalibration {
    #[serde(rename = "後板")]
    pub back_panel: f64,
    #[serde(rename = "ヒンジ")]
    pub hinge: f64,
}

/// Calculation constants from spec
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct Constants {
    pub plate_height_m: f64,
    pub plate_min_norm: f64,
    pub bottom_fill: f64,
    pub compression_ref_volume: f64,
    pub compression_factor: f64,
    pub effective_packing_min: f64,
    pub effective_packing_max: f64,
}

/// Material density entry
#[derive(Debug, Deserialize, Clone)]
pub struct MaterialEntry {
    pub density: f64,
}

/// Truck bed specification
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TruckSpec {
    pub bed_length: f64,
    pub bed_width: f64,
    pub bed_height: f64,
    pub level_volume: f64,
    pub heap_volume: f64,
    pub max_capacity: f64,
}

// === Accessor functions ===

/// Get material density by name, default to As殻 density
pub fn get_material_density(name: &str) -> f64 {
    SPEC.materials
        .get(name)
        .map(|m| m.density)
        .unwrap_or_else(|| {
            SPEC.materials.get("As殻").map(|m| m.density).unwrap_or(2.5)
        })
}

/// Get truck spec by class
pub fn get_truck_spec(truck_class: &str) -> Option<&TruckSpec> {
    SPEC.truck_specs.get(truck_class)
}

/// Get truck bed area (length * width)
pub fn get_truck_bed_area(truck_class: &str) -> f64 {
    SPEC.truck_specs
        .get(truck_class)
        .map(|s| s.bed_length * s.bed_width)
        .unwrap_or(default_bed_area())
}

/// Get default bed area (4t truck)
pub fn default_bed_area() -> f64 {
    SPEC.truck_specs
        .get("4t")
        .map(|s| s.bed_length * s.bed_width)
        .unwrap_or(6.8)
}

/// Get back panel (後板) calibration height
pub fn back_panel_height() -> f64 {
    SPEC.ranges.height.calibration.back_panel
}

/// Get hinge (ヒンジ) calibration height
pub fn hinge_height() -> f64 {
    SPEC.ranges.height.calibration.hinge
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_parses() {
        let spec = &*SPEC;
        assert_eq!(spec.version, "2.1.0");
        assert!(!spec.materials.is_empty());
        assert!(!spec.truck_specs.is_empty());
    }

    #[test]
    fn test_material_density() {
        assert!((get_material_density("As殻") - 2.5).abs() < f64::EPSILON);
        assert!((get_material_density("土砂") - 1.8).abs() < f64::EPSILON);
        // Unknown defaults to As殻
        assert!((get_material_density("unknown") - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_truck_bed_area() {
        let area_4t = get_truck_bed_area("4t");
        assert!((area_4t - 3.4 * 2.06).abs() < 0.01);
        // Unknown defaults to 4t bed area
        let default = default_bed_area();
        assert!((get_truck_bed_area("unknown") - default).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calibration_values() {
        assert!((back_panel_height() - 0.30).abs() < f64::EPSILON);
        assert!((hinge_height() - 0.60).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ranges() {
        let r = &SPEC.ranges;
        assert!(r.height.max > r.height.min);
        assert!((r.height.step - 0.05).abs() < f64::EPSILON);
        assert!((r.height.max - 0.8).abs() < f64::EPSILON);
        assert!(r.packing_density.min >= 0.0);
        assert!(r.packing_density.max <= 1.0);
        // New: taperRatio
        assert!((r.taper_ratio.min - 0.5).abs() < f64::EPSILON);
        assert!((r.taper_ratio.max - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_constants() {
        let c = &SPEC.constants;
        assert!((c.plate_height_m - 0.22).abs() < f64::EPSILON);
        assert!((c.bottom_fill - 0.9).abs() < f64::EPSILON);
        assert!((c.compression_ref_volume - 2.0).abs() < f64::EPSILON);
    }
}
