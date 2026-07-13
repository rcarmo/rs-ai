//! Test-for-test port of upstream `test/google-thinking-signature.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2): `isThinkingPart` + `retainThoughtSignature`.

#[cfg(test)]
mod tests {
    use crate::provider::google::{is_thinking_part, retain_thought_signature};
    use serde_json::json;

    #[test]
    fn treats_thought_true_as_thinking() {
        assert!(is_thinking_part(&json!({"thought": true})));
        assert!(is_thinking_part(
            &json!({"thought": true, "thoughtSignature": "opaque-signature"})
        ));
    }

    #[test]
    fn does_not_treat_thought_signature_alone_as_thinking() {
        assert!(!is_thinking_part(
            &json!({"thoughtSignature": "opaque-signature"})
        ));
        assert!(!is_thinking_part(
            &json!({"thought": false, "thoughtSignature": "opaque-signature"})
        ));
    }

    #[test]
    fn does_not_treat_empty_or_missing_signatures_as_thinking_if_thought_not_set() {
        assert!(!is_thinking_part(&json!({})));
        assert!(!is_thinking_part(
            &json!({"thought": false, "thoughtSignature": ""})
        ));
    }

    #[test]
    fn preserves_existing_signature_when_subsequent_deltas_omit_it() {
        let first = retain_thought_signature(None, Some("sig-1"));
        assert_eq!(first.as_deref(), Some("sig-1"));
        let second = retain_thought_signature(first.as_deref(), None);
        assert_eq!(second.as_deref(), Some("sig-1"));
        let third = retain_thought_signature(second.as_deref(), Some(""));
        assert_eq!(third.as_deref(), Some("sig-1"));
    }

    #[test]
    fn updates_signature_when_a_new_non_empty_signature_arrives() {
        assert_eq!(
            retain_thought_signature(Some("sig-1"), Some("sig-2")).as_deref(),
            Some("sig-2")
        );
    }
}
