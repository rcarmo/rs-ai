//! v0.99.1 unified typed model catalog coverage.

#[cfg(test)]
mod tests {
    use crate::model_catalog::{
        builtin_any_models, classify, generate_images, get_builtin_model_of_type,
        list_builtin_models,
    };
    use crate::types::{AnyModel, ClassifierContext, ClassifierStopReason, ModelType, StopReason};

    #[test]
    fn signed_typed_catalog_counts_and_cardinalities_match_schema_v6() {
        let all = builtin_any_models();
        assert_eq!(all.len(), 1615);
        let chat = list_builtin_models(Some(ModelType::Chat), None);
        let image = list_builtin_models(Some(ModelType::Image), None);
        let classifier = list_builtin_models(Some(ModelType::Classifier), None);
        assert_eq!(chat.len(), 1536);
        assert_eq!(image.len(), 59);
        assert_eq!(classifier.len(), 20);
        assert_eq!(
            chat.iter()
                .map(AnyModel::provider)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            41
        );
        assert_eq!(
            image
                .iter()
                .map(AnyModel::provider)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            1
        );
        assert_eq!(
            classifier
                .iter()
                .map(AnyModel::provider)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            5
        );
    }

    #[test]
    fn type_aware_lookup_keeps_chat_and_image_with_same_provider_id_separate() {
        let chat =
            get_builtin_model_of_type(ModelType::Chat, "openrouter", "google/gemini-3-pro-image")
                .unwrap();
        let image =
            get_builtin_model_of_type(ModelType::Image, "openrouter", "google/gemini-3-pro-image")
                .unwrap();
        assert_eq!(chat.model_type(), ModelType::Chat);
        assert_eq!(image.model_type(), ModelType::Image);
        assert_eq!(chat.id(), image.id());
        assert!(
            get_builtin_model_of_type(ModelType::Classifier, "typesafe", "jev-latest").is_some()
        );
        assert!(get_builtin_model_of_type(ModelType::Chat, "typesafe", "jev-latest").is_none());
    }

    #[test]
    fn typed_builtin_lookup_returns_isolated_models_and_stable_misses() {
        let mut chat = get_builtin_model_of_type(ModelType::Chat, "openai", "gpt-4o-mini").unwrap();
        let original_name = match &chat {
            AnyModel::Chat(model) => model.name.clone(),
            _ => unreachable!(),
        };
        match &mut chat {
            AnyModel::Chat(model) => model.name = "caller-mutated".into(),
            _ => unreachable!(),
        }
        let fresh = get_builtin_model_of_type(ModelType::Chat, "openai", "gpt-4o-mini").unwrap();
        assert_eq!(
            match fresh {
                AnyModel::Chat(model) => model.name,
                _ => unreachable!(),
            },
            original_name
        );
        assert!(get_builtin_model_of_type(ModelType::Chat, "openai", "missing").is_none());
        assert!(get_builtin_model_of_type(ModelType::Image, "openai", "gpt-4o-mini").is_none());
        assert!(
            get_builtin_model_of_type(ModelType::Classifier, "typesafe", "jev-latest").is_some()
        );
    }

    #[tokio::test]
    async fn wrong_typed_operations_return_typed_errors() {
        let chat = get_builtin_model_of_type(ModelType::Chat, "openai", "gpt-4o-mini").unwrap();
        let image = generate_images(
            &chat,
            &crate::images::ImagesContext { input: vec![] },
            &crate::images::openrouter::ImagesOptions::default(),
        )
        .await;
        assert_eq!(image.stop_reason, StopReason::Error);
        assert!(
            image
                .error_message
                .unwrap()
                .contains("is not an image model")
        );

        let result = classify(
            &chat,
            &ClassifierContext {
                state: serde_json::Map::new(),
                questions: indexmap::IndexMap::new(),
            },
            &crate::classifiers::ClassifierOptions::default(),
        )
        .await;
        assert_eq!(result.stop_reason, ClassifierStopReason::Error);
        assert!(
            result
                .error_message
                .unwrap()
                .contains("is not a classifier model")
        );
    }
}
