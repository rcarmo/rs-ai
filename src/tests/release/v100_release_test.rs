//! v1.0.0 release-pinned catalog and public-surface evidence.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::model_catalog::{builtin_any_models, get_builtin_model_of_type};
    use crate::provider::faux::stream_faux_text;
    use crate::registry;
    use crate::types::{ContentBlock, ModelType};
    use futures::StreamExt;

    #[test]
    fn signed_v100_catalog_delta_is_present() {
        for (provider, id) in [
            ("amazon-bedrock", "global.openai.gpt-6.1-sol"),
            ("opencode", "fledge-alpha-free"),
            ("openrouter", "apodex/apodex-1.1-mini:free"),
            ("openrouter", "typesafe/jev-router"),
            ("openrouter", "unbiased/pareto-26.10-preview"),
        ] {
            assert!(
                registry::get_model(provider, id).is_some(),
                "missing signed chat model {provider}/{id}"
            );
        }
        for removed in ["openai/gpt-6.1-sol-pro:batch", "openai/gpt-6.1-sol:batch"] {
            assert!(
                registry::get_model("openrouter", removed).is_none(),
                "removed batch alias retained: {removed}"
            );
        }
        let classifier =
            get_builtin_model_of_type(ModelType::Classifier, "vercel-ai-gateway", "liquid/d1")
                .expect("v1.0.0 classifier");
        let crate::types::AnyModel::Classifier(classifier) = classifier else {
            panic!("expected classifier model")
        };
        assert_eq!(classifier.context_window, 65_536);
        assert_eq!(classifier.cost.input, 0.04);
        assert_eq!(builtin_any_models().len(), 1615);
        assert_eq!(crate::models_generated::builtin_models().len(), 1536);
        assert_eq!(
            crate::images::models_generated::builtin_image_models().len(),
            59
        );
        assert_eq!(
            crate::classifier_models_generated::builtin_classifier_models().len(),
            20
        );
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
