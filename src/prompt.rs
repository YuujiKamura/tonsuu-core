//! Prompt access (deprecated)
//!
//! Prompts are now stored in prompt-spec.json and read at runtime by each consumer
//! (CLI reads from file, Web reads from JSON import).
//! This module is kept for backward compatibility but will be removed in a future version.

/// Deprecated: prompts are now in prompt-spec.json, read at runtime
#[deprecated(note = "Use prompt-spec.json directly. Prompts are no longer compiled into Rust.")]
pub fn build_core_prompt() -> String {
    String::from("DEPRECATED: read prompts from prompt-spec.json at runtime")
}

#[cfg(test)]
mod tests {
    #[allow(deprecated)]
    use super::*;

    #[test]
    fn test_deprecated_prompt_returns_message() {
        #[allow(deprecated)]
        let prompt = build_core_prompt();
        assert!(prompt.contains("DEPRECATED"));
    }
}
