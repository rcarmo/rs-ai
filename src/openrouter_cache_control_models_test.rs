//! Test-for-test adaptation of upstream `test/openrouter-cache-control-models.test.ts`.

#[cfg(test)]
mod tests {
    use crate::registry::get_model;

    #[test]
    fn enables_cache_control_for_openrouter_anthropic_latest_models() {
        for model_id in [
            "~anthropic/claude-fable-latest",
            "~anthropic/claude-haiku-latest",
            "~anthropic/claude-opus-latest",
            "~anthropic/claude-sonnet-latest",
        ] {
            let model = get_model("openrouter", model_id)
                .unwrap_or_else(|| panic!("openrouter/{model_id}"));
            assert_eq!(
                model.compat.cache_control_format.as_deref(),
                Some("anthropic"),
                "{model_id}"
            );
        }
    }
}
