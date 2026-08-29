//! Adaptation of the deterministic substance of upstream
//! `test/tool-call-id-normalization.test.ts` (`@earendil-works/pi-ai` v0.80.2,
//! regression for issue #1022): a pipe-separated, 450+ char tool-call id with
//! special characters (`+`, `/`, `=`) from the Responses API must normalize to a
//! provider-valid id when handed off to other providers. The upstream test is a
//! live cross-provider handoff; this guards the per-provider normalizers directly
//! against the exact failing id.

#[cfg(test)]
mod tests {
    use crate::provider::anthropic::normalize_anthropic_tool_call_id;
    use crate::provider::bedrock::normalize_bedrock_tool_call_id;
    use crate::provider::google::google_normalize_tool_call_id;
    use crate::provider::openai::normalize_tool_call_id;

    // The exact tool-call id from issue #1022 (call_id|base64-with-+/=).
    const FAILING: &str = "call_pAYbIr76hXIjncD9UE4eGfnS|t5nnb2qYMFWGSsr13fhCd1CaCu3t3qONEPuOudu4HSVEtA8YJSL6FAZUxvoOoD792VIJWl91g87EdqsCWp9krVsdBysQoDaf9lMCLb8BS4EYi4gQd5kBQBYLlgD71PYwvf+TbMD9J9/5OMD42oxSRj8H+vRf78/l2Xla33LWz4nOgsddBlbvabICRs8GHt5C9PK5keFtzyi3lsyVKNlfduK3iphsZqs4MLv4zyGJnvZo/+QzShyk5xnMSQX/f98+aEoNflEApCdEOXipipgeiNWnpFSHbcwmMkZoJhURNu+JEz3xCh1mrXeYoN5o+trLL3IXJacSsLYXDrYTipZZbJFRPAucgbnjYBC+/ZzJOfkwCs+Gkw7EoZR7ZQgJ8ma+9586n4tT4cI8DEhBSZsWMjrCt8dxKg==";

    fn matches_id_charset(id: &str) -> bool {
        !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    }

    #[test]
    fn anthropic_normalizes_to_valid_64_char_id() {
        let id = normalize_anthropic_tool_call_id(FAILING);
        assert!(
            matches_id_charset(&id),
            "anthropic id has invalid chars: {id}"
        );
        assert!(id.len() <= 64, "anthropic id must be <= 64: {}", id.len());
    }

    #[test]
    fn bedrock_normalizes_to_valid_64_char_id() {
        let id = normalize_bedrock_tool_call_id(FAILING);
        assert!(
            matches_id_charset(&id),
            "bedrock id has invalid chars: {id}"
        );
        assert!(id.len() <= 64, "bedrock id must be <= 64: {}", id.len());
    }

    #[test]
    fn openai_normalizes_pipe_id_to_valid_40_char_call_id() {
        let id = normalize_tool_call_id(FAILING, "openai");
        assert!(matches_id_charset(&id), "openai id has invalid chars: {id}");
        assert!(
            id.len() <= 40,
            "openai pipe call id must be <= 40: {}",
            id.len()
        );
        // The call-id portion (before `|`) is preserved.
        assert!(
            id.starts_with("call_pAYbIr76hXIjncD9UE4eGfnS"),
            "openai keeps the call_id part: {id}"
        );
    }

    #[test]
    fn google_normalizes_to_valid_64_char_id_for_id_requiring_models() {
        // Gemini tool-id normalization applies for claude-/gpt-oss- model ids.
        let id = google_normalize_tool_call_id("claude-sonnet-4-5", FAILING);
        assert!(matches_id_charset(&id), "google id has invalid chars: {id}");
        assert!(id.len() <= 64, "google id must be <= 64: {}", id.len());
    }
}
