// Ports src/main/services/paramCount.ts.
//
// Primary signal: `gguf.total` from the HF model-info response (?blobs=true), parsed
// server-side from the actual GGUF metadata and authoritative. Fallback: regex over the repo
// id / tags when that's absent.

use std::sync::LazyLock;

use regex::Regex;

use crate::ipc_types::ParamCountSource;

// The `regex` crate has no lookahead, unlike the original JS
// `/(\d+(?:\.\d+)?)\s*[Bb](?![a-zA-Z])/` (matches "8B" but not "8Billion" or "8Bit"). This
// loosely matches digits + optional decimal + optional whitespace + a B/M unit letter, and
// `first_unit_value` below manually checks the character right after the unit isn't itself a
// letter — replicating the negative lookahead by hand.
static UNIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*([BM])").expect("unit regex is well-formed")
});

/// First match of `text` for `unit` ('B' or 'M') whose following character isn't a letter, in
/// left-to-right order — same semantics as the JS regex this replaces.
fn first_unit_value(text: &str, unit: char) -> Option<f64> {
    for caps in UNIT_RE.captures_iter(text) {
        let matched_unit = caps.get(2).unwrap();
        if !matched_unit
            .as_str()
            .eq_ignore_ascii_case(&unit.to_string())
        {
            continue;
        }
        let next_is_letter = text[matched_unit.end()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic());
        if next_is_letter {
            continue;
        }
        if let Ok(value) = caps.get(1).unwrap().as_str().parse::<f64>() {
            return Some(value);
        }
    }
    None
}

pub fn resolve_param_count(
    repo_id: &str,
    tags: &[String],
    gguf_total: Option<i64>,
) -> (Option<i64>, ParamCountSource) {
    if let Some(total) = gguf_total {
        if total > 0 {
            return (Some(total), ParamCountSource::GgufMetadata);
        }
    }

    let candidates: Vec<&str> = std::iter::once(repo_id)
        .chain(tags.iter().map(String::as_str))
        .collect();

    for candidate in &candidates {
        if let Some(value) = first_unit_value(candidate, 'B') {
            return (
                Some((value * 1_000_000_000.0).round() as i64),
                ParamCountSource::RegexEstimate,
            );
        }
    }
    for candidate in &candidates {
        if let Some(value) = first_unit_value(candidate, 'M') {
            return (
                Some((value * 1_000_000.0).round() as i64),
                ParamCountSource::RegexEstimate,
            );
        }
    }

    (None, ParamCountSource::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_gguf_metadata_over_regex_when_present_and_positive() {
        let (count, source) = resolve_param_count("whatever-8B", &[], Some(4_628_569_635));
        assert_eq!(count, Some(4_628_569_635));
        assert_eq!(source, ParamCountSource::GgufMetadata);
    }

    #[test]
    fn falls_back_to_regex_when_gguf_metadata_missing_or_non_positive() {
        let (count, source) = resolve_param_count("Meta-Llama-3.1-8B-Instruct-GGUF", &[], None);
        assert_eq!(count, Some(8_000_000_000));
        assert_eq!(source, ParamCountSource::RegexEstimate);

        let (count, source) = resolve_param_count("Meta-Llama-3.1-8B-Instruct-GGUF", &[], Some(0));
        assert_eq!(count, Some(8_000_000_000));
        assert_eq!(source, ParamCountSource::RegexEstimate);
    }

    #[test]
    fn parses_decimal_billions() {
        let (count, _) = resolve_param_count("Qwen2.5-7.5B", &[], None);
        assert_eq!(count, Some(7_500_000_000));
    }

    #[test]
    fn falls_back_to_millions_when_no_billions_match_anywhere() {
        let (count, source) = resolve_param_count("mxbai-embed-large-500M", &[], None);
        assert_eq!(count, Some(500_000_000));
        assert_eq!(source, ParamCountSource::RegexEstimate);
    }

    #[test]
    fn billions_take_priority_over_millions_across_all_candidates() {
        // The 13B candidate must win even though it's tag[1] and an 8M candidate comes first —
        // resolve_param_count scans ALL candidates for B before trying any for M.
        let tags = vec!["8M-variant".to_string(), "13B-model".to_string()];
        let (count, _) = resolve_param_count("repo", &tags, None);
        assert_eq!(count, Some(13_000_000_000));
    }

    #[test]
    fn rejects_a_unit_letter_immediately_followed_by_another_letter() {
        // "8Bit" must not match as "8B" (no Rust regex lookahead — this exercises the manual
        // replacement for the JS `(?![a-zA-Z])` negative lookahead).
        let (count, source) = resolve_param_count("model-8Bit-quantized", &[], None);
        assert_eq!(count, None);
        assert_eq!(source, ParamCountSource::Unknown);
    }

    #[test]
    fn accepts_a_unit_letter_at_the_end_of_the_string() {
        let (count, _) = resolve_param_count("model-70B", &[], None);
        assert_eq!(count, Some(70_000_000_000));
    }

    #[test]
    fn unknown_when_no_candidate_matches_anything() {
        let (count, source) = resolve_param_count("some-repo-name", &[], None);
        assert_eq!(count, None);
        assert_eq!(source, ParamCountSource::Unknown);
    }
}
