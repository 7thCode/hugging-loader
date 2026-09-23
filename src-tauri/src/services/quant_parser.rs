// Ports src/main/services/quantParser.ts and the QUANT_TYPES vocabulary generation from
// src/shared/ipc-types.ts (lines 181-216 there).

use std::sync::LazyLock;

use regex::Regex;

// K-quants are named `Q<level>_K` or `Q<level>_K_<SUFFIX>` (S/M/L, plus Unsloth's extended
// XL/XXL "dynamic" quants, e.g. "Q8_K_XL"). Generated rather than enumerated by hand so new
// suffix combinations from quantizers (Unsloth, bartowski, ...) are recognized automatically.
const K_QUANT_LEVELS: [u8; 6] = [2, 3, 4, 5, 6, 8];
const K_QUANT_SUFFIXES: [&str; 6] = ["", "_S", "_M", "_L", "_XL", "_XXL"];

const BASE_QUANT_TYPES: [&str; 8] = ["F32", "F16", "BF16", "Q8_0", "Q5_1", "Q5_0", "Q4_1", "Q4_0"];

const EXTRA_QUANT_TYPES: [&str; 14] = [
    "IQ4_XS", "IQ4_NL", "IQ3_XXS", "IQ3_XS", "IQ3_S", "IQ3_M", "IQ2_XXS", "IQ2_XS", "IQ2_S",
    "IQ2_M", "IQ1_S", "IQ1_M", "TQ1_0", "TQ2_0",
];

/// Common llama.cpp GGUF quantization types, as the vocabulary a filename is matched against.
pub fn quant_types() -> Vec<String> {
    let mut types: Vec<String> = BASE_QUANT_TYPES.iter().map(|s| s.to_string()).collect();
    for level in K_QUANT_LEVELS {
        for suffix in K_QUANT_SUFFIXES {
            types.push(format!("Q{level}_K{suffix}"));
        }
    }
    types.extend(EXTRA_QUANT_TYPES.iter().map(|s| s.to_string()));
    types
}

// Longest-token-first alternation so e.g. Q5_K_M isn't matched as Q5_K's prefix stopping early.
static QUANT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    let mut types = quant_types();
    types.sort_by_key(|t| std::cmp::Reverse(t.len()));
    Regex::new(&format!(r"(?i)\b({})\b", types.join("|"))).expect("quant regex is well-formed")
});

pub fn parse_quant(filename: &str) -> Option<String> {
    QUANT_REGEX
        .captures(filename)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_common_quant_types() {
        assert_eq!(
            parse_quant("llama-2-7b-chat.Q4_K_M.gguf"),
            Some("Q4_K_M".to_string())
        );
        assert_eq!(parse_quant("model.q5_k_s.gguf"), Some("Q5_K_S".to_string()));
        assert_eq!(parse_quant("model-BF16.gguf"), Some("BF16".to_string()));
    }

    #[test]
    fn matches_unsloth_dynamic_xl_suffix() {
        assert_eq!(
            parse_quant("model-UD-Q8_K_XL.gguf"),
            Some("Q8_K_XL".to_string())
        );
    }

    #[test]
    fn prefers_longest_alternative_over_a_shorter_prefix() {
        // Q5_K_M must not be matched as Q5_K stopping early.
        assert_eq!(parse_quant("model.Q5_K_M.gguf"), Some("Q5_K_M".to_string()));
    }

    #[test]
    fn returns_none_when_no_quant_token_present() {
        assert_eq!(parse_quant("README.gguf"), None);
    }
}
