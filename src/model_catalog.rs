//! Unified typed model catalog and operation guards.

use crate::types::{AnyModel, ClassifierContext, ClassifierResult, ModelType, StreamOptions};

/// The model type, treating legacy chat models without a wire discriminator as chat.
pub fn get_model_type(model: &AnyModel) -> ModelType {
    model.model_type()
}

/// All signed built-in models across chat, image and classifier catalogs.
pub fn builtin_any_models() -> Vec<AnyModel> {
    let mut models = crate::models_generated::builtin_models()
        .into_iter()
        .map(|model| AnyModel::Chat(Box::new(model)))
        .collect::<Vec<_>>();
    models.extend(
        crate::images::models_generated::builtin_image_models()
            .into_iter()
            .map(AnyModel::Image),
    );
    models.extend(
        crate::classifier_models_generated::builtin_classifier_models()
            .into_iter()
            .map(AnyModel::Classifier),
    );
    models
}

/// Get one signed built-in model by type, provider and id.
pub fn get_builtin_model_of_type(
    model_type: ModelType,
    provider: &str,
    id: &str,
) -> Option<AnyModel> {
    match model_type {
        ModelType::Chat => crate::models_generated::builtin_models()
            .into_iter()
            .find(|model| model.provider == provider && model.id == id)
            .map(|model| AnyModel::Chat(Box::new(model))),
        ModelType::Image => crate::images::models_generated::builtin_image_models()
            .into_iter()
            .find(|model| model.provider == provider && model.id == id)
            .map(AnyModel::Image),
        ModelType::Classifier => crate::classifier_models_generated::builtin_classifier_models()
            .into_iter()
            .find(|model| model.provider == provider && model.id == id)
            .map(AnyModel::Classifier),
    }
}

/// List signed built-in models, optionally filtered by model type and provider.
pub fn list_builtin_models(model_type: Option<ModelType>, provider: Option<&str>) -> Vec<AnyModel> {
    builtin_any_models()
        .into_iter()
        .filter(|model| model_type.is_none_or(|value| model.model_type() == value))
        .filter(|model| provider.is_none_or(|value| model.provider() == value))
        .collect()
}

/// Compare complete model records after normalizing an omitted chat discriminator.
pub fn models_are_equal(left: &AnyModel, right: &AnyModel) -> bool {
    if left.model_type() != right.model_type() {
        return false;
    }
    serde_json::to_value(left).ok() == serde_json::to_value(right).ok()
}

/// Stream a chat model and return a typed error stream for other model kinds.
pub fn stream<'a>(
    model: &'a AnyModel,
    context: &'a crate::types::Context,
    options: &'a StreamOptions,
) -> crate::registry::EventStream<'a> {
    match model {
        AnyModel::Chat(model) => crate::registry::stream(model, context, options),
        _ => {
            let model_id = format!("{}/{}", model.provider(), model.id());
            Box::pin(tokio_stream::once(crate::events::Event::Error {
                reason: crate::types::StopReason::Error,
                error: std::sync::Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                    format!("Model {model_id} is not a chat model"),
                )),
                message: None,
            }))
        }
    }
}

/// Generate images through the unified typed operation.
pub async fn generate_images(
    model: &AnyModel,
    context: &crate::images::ImagesContext,
    options: &crate::images::openrouter::ImagesOptions,
) -> crate::images::AssistantImages {
    match model {
        AnyModel::Image(model) => crate::images::generate_images(model, context, options).await,
        _ => crate::images::AssistantImages {
            api: String::new(),
            provider: model.provider().to_string(),
            model: model.id().to_string(),
            output: Vec::new(),
            stop_reason: crate::types::StopReason::Error,
            timestamp: crate::utils::now_millis(),
            response_id: None,
            usage: None,
            error_message: Some(format!(
                "Model {}/{} is not an image model",
                model.provider(),
                model.id()
            )),
        },
    }
}

/// Classify through the unified typed operation.
pub async fn classify(
    model: &AnyModel,
    context: &ClassifierContext,
    options: &crate::classifiers::ClassifierOptions,
) -> ClassifierResult {
    match model {
        AnyModel::Classifier(model) => crate::classifiers::classify(model, context, options).await,
        _ => ClassifierResult {
            api: String::new(),
            provider: model.provider().to_string(),
            model: model.id().to_string(),
            answers: indexmap::IndexMap::new(),
            usage: None,
            stop_reason: crate::types::ClassifierStopReason::Error,
            error_message: Some(format!(
                "Model {}/{} is not a classifier model",
                model.provider(),
                model.id()
            )),
            timestamp: crate::utils::now_millis(),
        },
    }
}
