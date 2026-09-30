//! v0.99.2 release-pinned catalog and public-surface evidence.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::model_catalog::{builtin_any_models, get_builtin_model_of_type};
    use crate::provider::faux::stream_faux_text;
    use crate::registry;
    use crate::types::{ContentBlock, ModelType};
    use futures::StreamExt;

    #[test]
    fn signed_v0992_catalog_additions_are_present() {
        for (provider, id) in [
            ("amazon-bedrock", "anthropic.claude-sonnet-5-5"),
            ("amazon-bedrock", "openai.gpt-6.1-sol"),
            ("amazon-bedrock", "us.openai.gpt-6.1-sol"),
            ("baseten", "deepseek-ai/DeepSeek-V4.1-Flash-Fast"),
            ("github-copilot", "gpt-6.1-sol"),
            ("opencode", "gpt-6.1-sol"),
        ] {
            assert!(
                registry::get_model(provider, id).is_some(),
                "missing signed chat model {provider}/{id}"
            );
        }
        for (provider, id) in [
            ("openrouter", "inception/mercury-decide:free"),
            ("openrouter", "togethercomputer/tev1-4b-experimental"),
            ("vercel-ai-gateway", "liquid/d1"),
        ] {
            assert!(
                get_builtin_model_of_type(ModelType::Classifier, provider, id).is_some(),
                "missing signed classifier model {provider}/{id}"
            );
        }
        assert_eq!(builtin_any_models().len(), 1601);
    }

    #[tokio::test]
    async fn rust_registry_and_faux_stream_are_available_without_node_loader_contract() {
        registry::register_builtin_models();
        let model = registry::get_model("openai", "gpt-4o-mini").expect("built-in model");
        assert_eq!(model.api, crate::types::api::OPENAI_RESPONSES);
        let mut stream = stream_faux_text("OK", &model);
        let mut text = String::new();
        let mut done = false;
        while let Some(event) = stream.next().await {
            match event {
                Event::TextDelta { delta } => text.push_str(&delta),
                Event::Done { message, .. } => {
                    done = message.content.iter().any(
                        |block| matches!(block, ContentBlock::Text { text, .. } if text == "OK"),
                    );
                }
                _ => {}
            }
        }
        assert_eq!(text, "OK");
        assert!(done);
    }
}
