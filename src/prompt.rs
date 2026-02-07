//! Prompt building from prompt-spec.json
//!
//! Constructs AI prompts by interpolating the spec's jsonTemplate
//! and rangeGuide into the promptFormat template.

use crate::spec::SPEC;

/// Build the core estimation prompt from prompt-spec.json
///
/// Replaces `{jsonTemplate}` and `{rangeGuide}` placeholders in
/// the promptFormat string.
pub fn build_core_prompt() -> String {
    let template_json = serde_json::to_string(&SPEC.json_template)
        .unwrap_or_else(|_| "{}".to_string());

    SPEC.prompt_format
        .replace("{jsonTemplate}", &template_json)
        .replace("{rangeGuide}", &SPEC.range_guide)
}

/// Get the raw range guide string
pub fn range_guide() -> &'static str {
    &SPEC.range_guide
}

/// Get the JSON template as a string
pub fn json_template_string() -> String {
    serde_json::to_string(&SPEC.json_template)
        .unwrap_or_else(|_| "{}".to_string())
}

/// Get the JSON template as pretty-printed string (for display)
pub fn json_template_pretty() -> String {
    serde_json::to_string_pretty(&SPEC.json_template)
        .unwrap_or_else(|_| "{}".to_string())
}

/// WASM-friendly version
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "buildCorePrompt")]
pub fn build_core_prompt_wasm() -> String {
    build_core_prompt()
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "getRangeGuide")]
pub fn range_guide_wasm() -> String {
    SPEC.range_guide.clone()
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "getJsonTemplate")]
pub fn json_template_wasm() -> String {
    json_template_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_core_prompt_contains_json_template() {
        let prompt = build_core_prompt();
        // Should contain actual JSON values, not placeholders
        assert!(!prompt.contains("{jsonTemplate}"));
        assert!(!prompt.contains("{rangeGuide}"));
        // Should contain recognizable content
        assert!(prompt.contains("isTargetDetected"));
        assert!(prompt.contains("upperArea"));
        assert!(prompt.contains("Output ONLY JSON"));
    }

    #[test]
    fn test_build_core_prompt_contains_range_guide() {
        let prompt = build_core_prompt();
        // Range guide should have calibration references
        assert!(prompt.contains("後板"));
        assert!(prompt.contains("ヒンジ"));
    }

    #[test]
    fn test_json_template_string_valid_json() {
        let json_str = json_template_string();
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json_str);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_range_guide_not_empty() {
        assert!(!range_guide().is_empty());
    }
}
