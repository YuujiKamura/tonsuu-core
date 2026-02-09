//! Box-overlay analysis pipeline
//!
//! Provides the `AiBackend` trait and `analyze_box_overlay` function that
//! encapsulates the full ensemble geometry + fill estimation flow.
//! This ensures CLI and Web produce identical results from the same AI responses.

use crate::calculation::{calculate_tonnage, height_from_geometry, CoreParams};
use crate::parse::{parse_fill, parse_geometry, FillResponse, GeometryResponse, ParseError};
use crate::spec::SPEC;

use std::fmt;

// ─── Errors ──────────────────────────────────────────────────────────

/// Pipeline error
#[derive(Debug)]
pub enum PipelineError {
    /// AI backend returned an error
    AiError(String),
    /// JSON parse failure
    ParseError(String),
    /// All geometry ensemble runs failed
    NoValidGeometry,
    /// All fill ensemble runs failed
    NoValidFill,
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AiError(s) => write!(f, "AI error: {}", s),
            Self::ParseError(s) => write!(f, "Parse error: {}", s),
            Self::NoValidGeometry => write!(f, "幾何学検出が全ての試行で失敗しました"),
            Self::NoValidFill => write!(f, "充填率推定が全ての試行で失敗しました"),
        }
    }
}

impl std::error::Error for PipelineError {}

impl From<ParseError> for PipelineError {
    fn from(e: ParseError) -> Self {
        Self::ParseError(e.message)
    }
}

// ─── AiBackend trait ─────────────────────────────────────────────────

/// Trait for sending prompts to an AI model.
/// Implemented differently by CLI (Gemini CLI subprocess) and Web (Google GenAI SDK).
pub trait AiBackend {
    /// Send a text prompt with image data and return the raw text response.
    fn send_prompt(&self, prompt: &str, images: &[Vec<u8>]) -> Result<String, PipelineError>;
}

// ─── Config / Result types ───────────────────────────────────────────

/// Configuration for box-overlay analysis
#[derive(Debug, Clone)]
pub struct BoxOverlayConfig {
    pub truck_class: String,
    pub material_type: String,
    /// Number of ensemble runs (typically 2-3)
    pub ensemble_count: usize,
}

/// Full result of a box-overlay analysis
#[derive(Debug, Clone)]
pub struct BoxOverlayResult {
    pub height_m: f64,
    pub fill_ratio_l: f64,
    pub fill_ratio_w: f64,
    pub taper_ratio: f64,
    pub packing_density: f64,
    pub effective_packing: f64,
    pub volume: f64,
    pub tonnage: f64,
    pub density: f64,
    pub material_type: String,
    pub reasoning: String,
    pub geometry_runs: Vec<GeometryRunLog>,
    pub fill_runs: Vec<FillRunLog>,
}

/// Log of a single geometry detection run
#[derive(Debug, Clone)]
pub struct GeometryRunLog {
    pub raw_response: String,
    pub parsed: Option<GeometryResponse>,
    pub scale_method: String,
    pub height_m: f64,
}

/// Log of a single fill estimation run
#[derive(Debug, Clone)]
pub struct FillRunLog {
    pub raw_response: String,
    pub parsed: Option<FillResponse>,
}

// ─── Pipeline ────────────────────────────────────────────────────────

/// Run the full box-overlay analysis pipeline.
///
/// 1. Geometry detection (ensemble) -> median height
/// 2. Fill estimation (ensemble) -> average fill ratios (clamped to SPEC ranges)
/// 3. Tonnage calculation
///
/// Matches the logic in `boxOverlayService.ts::analyzeBoxOverlayEnsemble`.
pub fn analyze_box_overlay(
    backend: &dyn AiBackend,
    images: &[Vec<u8>],
    config: &BoxOverlayConfig,
) -> Result<BoxOverlayResult, PipelineError> {
    let spec = &*SPEC;
    let ranges = &spec.ranges;

    let bed_height = spec
        .truck_specs
        .get(&config.truck_class)
        .map(|s| s.bed_height)
        .unwrap_or(0.32);

    // ── Step 1: Geometry detection (ensemble, take median of height_m) ──

    let mut height_list = Vec::new();
    let mut geometry_runs = Vec::new();

    for _i in 0..config.ensemble_count {
        match backend.send_prompt(&spec.geometry_prompt, images) {
            Ok(response) => match parse_geometry(&response) {
                Ok(geo) => {
                    if geo.tailgate_top_y <= 0.0 {
                        geometry_runs.push(GeometryRunLog {
                            raw_response: response,
                            parsed: Some(geo),
                            scale_method: "none".into(),
                            height_m: 0.0,
                        });
                        continue;
                    }

                    let (h, method) = height_from_geometry(
                        geo.tailgate_top_y,
                        geo.tailgate_bottom_y,
                        geo.cargo_top_y,
                        geo.plate_box,
                        bed_height,
                    );

                    if method == "none" {
                        geometry_runs.push(GeometryRunLog {
                            raw_response: response,
                            parsed: Some(geo),
                            scale_method: "none".into(),
                            height_m: 0.0,
                        });
                        continue;
                    }

                    height_list.push(h);
                    geometry_runs.push(GeometryRunLog {
                        raw_response: response,
                        parsed: Some(geo),
                        scale_method: method.to_string(),
                        height_m: h,
                    });
                }
                Err(_e) => {
                    geometry_runs.push(GeometryRunLog {
                        raw_response: response,
                        parsed: None,
                        scale_method: "parse_error".into(),
                        height_m: 0.0,
                    });
                }
            },
            Err(_e) => {
                geometry_runs.push(GeometryRunLog {
                    raw_response: String::new(),
                    parsed: None,
                    scale_method: "error".into(),
                    height_m: 0.0,
                });
            }
        }
    }

    if height_list.is_empty() {
        return Err(PipelineError::NoValidGeometry);
    }

    let height_m = median(&height_list);

    // ── Step 2: Fill estimation (ensemble, average, clamp) ──

    let mut fill_l_list = Vec::new();
    let mut fill_w_list = Vec::new();
    let mut taper_list = Vec::new();
    let mut packing_list = Vec::new();
    let mut last_reasoning = String::new();
    let mut detected_materials: Vec<String> = Vec::new();
    let mut fill_runs = Vec::new();

    for _i in 0..config.ensemble_count {
        match backend.send_prompt(&spec.fill_prompt, images) {
            Ok(response) => match parse_fill(&response) {
                Ok(fill) => {
                    fill_l_list.push(fill.fill_ratio_l);
                    fill_w_list.push(fill.fill_ratio_w);
                    taper_list.push(fill.taper_ratio);
                    packing_list.push(fill.packing_density);
                    if let Some(ref m) = fill.material_type {
                        if !m.is_empty() && m != "?" {
                            detected_materials.push(m.clone());
                        }
                    }
                    if let Some(ref r) = fill.reasoning {
                        last_reasoning = r.clone();
                    }
                    fill_runs.push(FillRunLog {
                        raw_response: response,
                        parsed: Some(fill),
                    });
                }
                Err(_e) => {
                    fill_runs.push(FillRunLog {
                        raw_response: response,
                        parsed: None,
                    });
                }
            },
            Err(_e) => {
                fill_runs.push(FillRunLog {
                    raw_response: String::new(),
                    parsed: None,
                });
            }
        }
    }

    if fill_l_list.is_empty() {
        return Err(PipelineError::NoValidFill);
    }

    let fill_l = average(&fill_l_list).clamp(ranges.fill_ratio_l.min, ranges.fill_ratio_l.max);
    let fill_w = average(&fill_w_list).clamp(ranges.fill_ratio_w.min, ranges.fill_ratio_w.max);
    let taper = average(&taper_list).clamp(ranges.taper_ratio.min, ranges.taper_ratio.max);
    let packing = average(&packing_list).clamp(ranges.packing_density.min, ranges.packing_density.max);

    // ── Step 3: Calculate tonnage ──

    // Use AI-detected material if available, otherwise fall back to config
    let material_type = mode_string(&detected_materials)
        .unwrap_or_else(|| config.material_type.clone());

    let params = CoreParams {
        height: height_m,
        fill_ratio_l: fill_l,
        fill_ratio_w: fill_w,
        taper_ratio: taper,
        packing_density: packing,
        material_type,
    };

    let calc = calculate_tonnage(&params, Some(&config.truck_class));

    Ok(BoxOverlayResult {
        height_m: round3(height_m),
        fill_ratio_l: round3(fill_l),
        fill_ratio_w: round3(fill_w),
        taper_ratio: round3(taper),
        packing_density: round3(calc.effective_packing),
        effective_packing: round3(calc.effective_packing),
        volume: round4(calc.volume),
        tonnage: round2(calc.tonnage),
        density: calc.density,
        material_type: params.material_type,
        reasoning: last_reasoning,
        geometry_runs,
        fill_runs,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn median(arr: &[f64]) -> f64 {
    let mut sorted = arr.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[sorted.len() / 2]
}

fn average(arr: &[f64]) -> f64 {
    arr.iter().sum::<f64>() / arr.len() as f64
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

/// Get most common string from a list (mode). Returns None if empty.
fn mode_string(values: &[String]) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let mut counts = std::collections::HashMap::new();
    for v in values {
        *counts.entry(v.as_str()).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(v, _)| v.to_string())
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock AI backend that returns predefined responses
    struct MockBackend {
        geometry_responses: Vec<String>,
        fill_responses: Vec<String>,
        geo_call: std::cell::Cell<usize>,
        fill_call: std::cell::Cell<usize>,
    }

    impl MockBackend {
        fn new(geo: Vec<&str>, fill: Vec<&str>) -> Self {
            Self {
                geometry_responses: geo.into_iter().map(String::from).collect(),
                fill_responses: fill.into_iter().map(String::from).collect(),
                geo_call: std::cell::Cell::new(0),
                fill_call: std::cell::Cell::new(0),
            }
        }
    }

    impl AiBackend for MockBackend {
        fn send_prompt(&self, prompt: &str, _images: &[Vec<u8>]) -> Result<String, PipelineError> {
            // Distinguish geometry vs fill by checking prompt content
            if prompt.contains("tailgateTopY") {
                let idx = self.geo_call.get();
                self.geo_call.set(idx + 1);
                if idx < self.geometry_responses.len() {
                    Ok(self.geometry_responses[idx].clone())
                } else {
                    // Cycle last response
                    Ok(self.geometry_responses.last().unwrap().clone())
                }
            } else {
                let idx = self.fill_call.get();
                self.fill_call.set(idx + 1);
                if idx < self.fill_responses.len() {
                    Ok(self.fill_responses[idx].clone())
                } else {
                    Ok(self.fill_responses.last().unwrap().clone())
                }
            }
        }
    }

    #[test]
    fn test_full_pipeline_mock() {
        let geo_json = r#"{"plateBox":[0.4,0.7,0.6,0.84],"tailgateTopY":0.3,"tailgateBottomY":0.5,"cargoTopY":0.2}"#;
        let fill_json = r#"{"fillRatioL":0.8,"fillRatioW":0.85,"taperRatio":0.9,"packingDensity":0.8,"reasoning":"Well packed"}"#;

        let backend = MockBackend::new(vec![geo_json, geo_json], vec![fill_json, fill_json]);
        let config = BoxOverlayConfig {
            truck_class: "4t".to_string(),
            material_type: "As殻".to_string(),
            ensemble_count: 2,
        };

        let result = analyze_box_overlay(&backend, &[vec![1, 2, 3]], &config).unwrap();

        assert!(result.height_m > 0.0, "height should be > 0");
        assert!(result.tonnage > 0.0, "tonnage should be > 0");
        assert!(result.volume > 0.0, "volume should be > 0");
        assert_eq!(result.geometry_runs.len(), 2);
        assert_eq!(result.fill_runs.len(), 2);
        assert_eq!(result.reasoning, "Well packed");
        assert!((result.density - 2.5).abs() < f64::EPSILON, "As殻 density = 2.5");
    }

    #[test]
    fn test_pipeline_geometry_all_fail() {
        let backend = MockBackend::new(
            vec!["not json at all", "also bad"],
            vec![r#"{"fillRatioL":0.8,"fillRatioW":0.85,"taperRatio":0.9,"packingDensity":0.8}"#],
        );
        let config = BoxOverlayConfig {
            truck_class: "4t".to_string(),
            material_type: "As殻".to_string(),
            ensemble_count: 2,
        };

        let result = analyze_box_overlay(&backend, &[], &config);
        assert!(matches!(result, Err(PipelineError::NoValidGeometry)));
    }

    #[test]
    fn test_pipeline_fill_all_fail() {
        let geo_json = r#"{"tailgateTopY":0.3,"tailgateBottomY":0.5,"cargoTopY":0.2}"#;
        let backend = MockBackend::new(vec![geo_json], vec!["bad fill"]);
        let config = BoxOverlayConfig {
            truck_class: "4t".to_string(),
            material_type: "As殻".to_string(),
            ensemble_count: 1,
        };

        let result = analyze_box_overlay(&backend, &[], &config);
        assert!(matches!(result, Err(PipelineError::NoValidFill)));
    }

    #[test]
    fn test_pipeline_partial_geometry_success() {
        // First run fails, second succeeds
        let good_geo = r#"{"tailgateTopY":0.3,"tailgateBottomY":0.5,"cargoTopY":0.2}"#;
        let fill_json = r#"{"fillRatioL":0.8,"fillRatioW":0.85,"taperRatio":0.9,"packingDensity":0.8}"#;

        let backend = MockBackend::new(vec!["bad json", good_geo], vec![fill_json, fill_json]);
        let config = BoxOverlayConfig {
            truck_class: "4t".to_string(),
            material_type: "As殻".to_string(),
            ensemble_count: 2,
        };

        let result = analyze_box_overlay(&backend, &[], &config).unwrap();
        assert!(result.height_m > 0.0);
        // One failed, one succeeded
        assert!(result.geometry_runs[0].parsed.is_none());
        assert!(result.geometry_runs[1].parsed.is_some());
    }

    #[test]
    fn test_pipeline_clamps_fill_to_spec_ranges() {
        let geo_json = r#"{"tailgateTopY":0.3,"tailgateBottomY":0.5,"cargoTopY":0.2}"#;
        // AI returns out-of-range fill values
        let fill_json = r#"{"fillRatioL":0.1,"fillRatioW":0.99,"taperRatio":0.2,"packingDensity":0.99}"#;

        let backend = MockBackend::new(vec![geo_json], vec![fill_json]);
        let config = BoxOverlayConfig {
            truck_class: "4t".to_string(),
            material_type: "As殻".to_string(),
            ensemble_count: 1,
        };

        let result = analyze_box_overlay(&backend, &[], &config).unwrap();
        let r = &SPEC.ranges;
        assert!(result.fill_ratio_l >= r.fill_ratio_l.min);
        assert!(result.fill_ratio_w <= r.fill_ratio_w.max);
        assert!(result.taper_ratio >= r.taper_ratio.min);
    }

    #[test]
    fn test_pipeline_result_matches_ts_calculation() {
        // Use known geometry: tailgate top=0.3, bot=0.5, cargo_top=0.2, bed_height=0.32
        // height_from_geometry: tg_height_norm=0.2, m_per_norm=1.6, h=(0.5-0.2)*1.6=0.48
        let geo_json = r#"{"tailgateTopY":0.3,"tailgateBottomY":0.5,"cargoTopY":0.2}"#;
        let fill_json = r#"{"fillRatioL":0.8,"fillRatioW":0.85,"taperRatio":0.9,"packingDensity":0.8}"#;

        let backend = MockBackend::new(vec![geo_json], vec![fill_json]);
        let config = BoxOverlayConfig {
            truck_class: "4t".to_string(),
            material_type: "As殻".to_string(),
            ensemble_count: 1,
        };

        let result = analyze_box_overlay(&backend, &[], &config).unwrap();

        // height should be 0.48
        assert!(
            (result.height_m - 0.48).abs() < 0.01,
            "height={}, expected ~0.48",
            result.height_m
        );

        // Manual: bedL=3.4, bedW=2.06
        // effectiveL = 0.8 * 0.9 = 0.72
        // effectiveW = (0.9 + 0.85) / 2 = 0.875
        // volume = 3.4 * 2.06 * 0.48 * 0.72 * 0.875 = ~2.117
        assert!(result.volume > 2.0 && result.volume < 2.3, "volume={}", result.volume);
        // tonnage should be in a reasonable range
        assert!(result.tonnage > 3.0 && result.tonnage < 5.0, "tonnage={}", result.tonnage);
    }

    #[test]
    fn test_median_odd() {
        assert!((median(&[3.0, 1.0, 2.0]) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_median_even() {
        // Our median takes sorted[len/2], so for [1,2,3,4] -> sorted[2] = 3
        assert!((median(&[4.0, 1.0, 3.0, 2.0]) - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_average() {
        assert!((average(&[1.0, 2.0, 3.0]) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pipeline_invalid_tailgate_top_skipped() {
        // tailgateTopY = 0 should be skipped (invalid)
        let bad_geo = r#"{"tailgateTopY":0.0,"tailgateBottomY":0.5,"cargoTopY":0.2}"#;
        let good_geo = r#"{"tailgateTopY":0.3,"tailgateBottomY":0.5,"cargoTopY":0.2}"#;
        let fill_json = r#"{"fillRatioL":0.8,"fillRatioW":0.85,"taperRatio":0.9,"packingDensity":0.8}"#;

        let backend = MockBackend::new(vec![bad_geo, good_geo], vec![fill_json, fill_json]);
        let config = BoxOverlayConfig {
            truck_class: "4t".to_string(),
            material_type: "As殻".to_string(),
            ensemble_count: 2,
        };

        let result = analyze_box_overlay(&backend, &[], &config).unwrap();
        assert!(result.height_m > 0.0);
        assert_eq!(result.geometry_runs[0].scale_method, "none");
        assert_ne!(result.geometry_runs[1].scale_method, "none");
    }
}
