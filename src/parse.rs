//! AI response JSON parsing
//!
//! Extracts and parses JSON from AI model responses, handling cases where
//! the response contains extra text around the JSON object.

use std::fmt;

/// Parse error
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

/// Geometry detection response from AI
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryResponse {
    #[serde(default)]
    pub plate_box: Option<[f64; 4]>,
    #[serde(default)]
    pub tailgate_top_y: f64,
    #[serde(default)]
    pub tailgate_bottom_y: f64,
    #[serde(default)]
    pub cargo_top_y: f64,
}

/// Fill estimation response from AI
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillResponse {
    #[serde(default = "default_fill_l")]
    pub fill_ratio_l: f64,
    #[serde(default = "default_fill_w")]
    pub fill_ratio_w: f64,
    #[serde(default = "default_taper")]
    pub taper_ratio: f64,
    #[serde(default = "default_packing")]
    pub packing_density: f64,
    #[serde(default)]
    pub reasoning: Option<String>,
}

fn default_fill_l() -> f64 { 0.8 }
fn default_fill_w() -> f64 { 0.7 }
fn default_taper() -> f64 { 0.85 }
fn default_packing() -> f64 { 0.7 }

/// Extract and parse JSON from AI response text.
///
/// First tries direct parse. On failure, extracts the first `{...}` block
/// (respecting string literals and nested braces) and parses that.
/// Matches the TypeScript `parseJsonSafe` function in boxOverlayService.ts.
pub fn parse_json_safe<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, ParseError> {
    // Try direct parse first
    if let Ok(v) = serde_json::from_str(text) {
        return Ok(v);
    }

    // Extract first JSON object
    let start = text.find('{').ok_or_else(|| ParseError {
        message: "JSONオブジェクトが見つかりません".to_string(),
    })?;

    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;

    for i in start..bytes.len() {
        let ch = bytes[i];
        if escape {
            escape = false;
            continue;
        }
        if ch == b'\\' && in_string {
            escape = true;
            continue;
        }
        if ch == b'"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if ch == b'{' {
            depth += 1;
        } else if ch == b'}' {
            depth -= 1;
        }
        if depth == 0 {
            let extracted = &text[start..=i];
            return serde_json::from_str(extracted).map_err(|e| ParseError {
                message: format!("JSON抽出後もパース失敗: {}", e),
            });
        }
    }

    Err(ParseError {
        message: "不完全なJSONオブジェクト".to_string(),
    })
}

/// Parse a geometry detection response
pub fn parse_geometry(text: &str) -> Result<GeometryResponse, ParseError> {
    parse_json_safe(text)
}

/// Parse a fill estimation response
pub fn parse_fill(text: &str) -> Result<FillResponse, ParseError> {
    parse_json_safe(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_geometry_clean_json() {
        let json = r#"{"plateBox":[0.4,0.7,0.6,0.84],"tailgateTopY":0.3,"tailgateBottomY":0.5,"cargoTopY":0.2}"#;
        let geo = parse_geometry(json).unwrap();
        assert!((geo.tailgate_top_y - 0.3).abs() < f64::EPSILON);
        assert!((geo.tailgate_bottom_y - 0.5).abs() < f64::EPSILON);
        assert!((geo.cargo_top_y - 0.2).abs() < f64::EPSILON);
        let pb = geo.plate_box.unwrap();
        assert!((pb[0] - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_geometry_with_extra_text() {
        let text = r#"Here is the JSON result:
{"plateBox":null,"tailgateTopY":0.35,"tailgateBottomY":0.55,"cargoTopY":0.25}
Some trailing text"#;
        let geo = parse_geometry(text).unwrap();
        assert!((geo.tailgate_top_y - 0.35).abs() < f64::EPSILON);
        assert!(geo.plate_box.is_none());
    }

    #[test]
    fn test_parse_geometry_empty_response() {
        let result = parse_geometry("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_geometry_no_json() {
        let result = parse_geometry("This is not JSON at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fill_clean_json() {
        let json = r#"{"fillRatioL":0.8,"fillRatioW":0.85,"taperRatio":0.9,"packingDensity":0.8,"reasoning":"Cargo is well packed"}"#;
        let fill = parse_fill(json).unwrap();
        assert!((fill.fill_ratio_l - 0.8).abs() < f64::EPSILON);
        assert!((fill.fill_ratio_w - 0.85).abs() < f64::EPSILON);
        assert!((fill.taper_ratio - 0.9).abs() < f64::EPSILON);
        assert!((fill.packing_density - 0.8).abs() < f64::EPSILON);
        assert_eq!(fill.reasoning.as_deref(), Some("Cargo is well packed"));
    }

    #[test]
    fn test_parse_fill_missing_fields_use_defaults() {
        let json = r#"{"fillRatioL":0.75}"#;
        let fill = parse_fill(json).unwrap();
        assert!((fill.fill_ratio_l - 0.75).abs() < f64::EPSILON);
        // Defaults
        assert!((fill.fill_ratio_w - 0.7).abs() < f64::EPSILON);
        assert!((fill.taper_ratio - 0.85).abs() < f64::EPSILON);
        assert!((fill.packing_density - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_fill_with_extra_text() {
        let text = r#"```json
{"fillRatioL":0.82,"fillRatioW":0.78,"taperRatio":0.88,"packingDensity":0.75,"reasoning":"test"}
```"#;
        let fill = parse_fill(text).unwrap();
        assert!((fill.fill_ratio_l - 0.82).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_json_safe_nested_braces_in_string() {
        // JSON with braces inside string values should be handled correctly
        let text = r#"{"fillRatioL":0.8,"fillRatioW":0.85,"taperRatio":0.9,"packingDensity":0.8,"reasoning":"cargo {heavy} and {packed}"}"#;
        let fill: FillResponse = parse_json_safe(text).unwrap();
        assert_eq!(fill.reasoning.as_deref(), Some("cargo {heavy} and {packed}"));
    }

    #[test]
    fn test_parse_incomplete_json() {
        let text = r#"{"fillRatioL":0.8,"fillRatioW":0.85"#;
        let result: Result<FillResponse, _> = parse_json_safe(text);
        assert!(result.is_err());
    }
}
