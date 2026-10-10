//! Unified classifier model operations and provider registry.

use crate::types::{ClassifierContext, ClassifierModel, ClassifierResult, ClassifierStopReason};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Once, RwLock};
use std::time::Duration;

pub mod llama_cpp;
pub mod openai_decisions;
mod shared;
pub mod system_one;

pub type ClassifierPayloadHook = Arc<
    dyn Fn(Value, &ClassifierModel) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>
        + Send
        + Sync,
>;
pub type ClassifierResponseHook =
    Arc<dyn Fn(u16, &HashMap<String, String>, &ClassifierModel) + Send + Sync>;

#[derive(Clone, Default)]
pub struct ClassifierOptions {
    pub api_key: Option<String>,
    /// Case-insensitive request header overlay. `None` removes a model header.
    pub headers: Option<HashMap<String, Option<String>>>,
    pub timeout: Option<Duration>,
    pub max_retries: Option<u32>,
    pub max_retry_delay_ms: Option<u64>,
    pub temperature: Option<f64>,
    pub env: Option<HashMap<String, String>>,
    pub cancel: Option<tokio::sync::watch::Receiver<bool>>,
    pub on_payload: Option<ClassifierPayloadHook>,
    pub on_response: Option<ClassifierResponseHook>,
}

impl std::fmt::Debug for ClassifierOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassifierOptions")
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("headers", &self.headers)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("max_retry_delay_ms", &self.max_retry_delay_ms)
            .field("temperature", &self.temperature)
            .field("env", &self.env)
            .field("cancel", &self.cancel.as_ref().map(|_| "..."))
            .field("on_payload", &self.on_payload.as_ref().map(|_| "<hook>"))
            .field("on_response", &self.on_response.as_ref().map(|_| "<hook>"))
            .finish()
    }
}

pub trait ClassifierApiProvider: Send + Sync {
    fn api(&self) -> &str;
    fn classify<'a>(
        &'a self,
        model: &'a ClassifierModel,
        context: &'a ClassifierContext,
        options: &'a ClassifierOptions,
    ) -> Pin<Box<dyn Future<Output = ClassifierResult> + Send + 'a>>;
}

static CLASSIFIER_APIS: LazyLock<RwLock<HashMap<String, Arc<dyn ClassifierApiProvider>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static CLASSIFIER_MODELS: LazyLock<RwLock<HashMap<String, ClassifierModel>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static REGISTER_BUILTINS: Once = Once::new();

fn key(provider: &str, id: &str) -> String {
    format!("{provider}/{id}")
}

fn ensure_builtins() {
    REGISTER_BUILTINS.call_once(|| {
        register_builtin_classifier_providers();
        register_builtin_classifier_models();
    });
}

pub fn register_classifier_api(provider: Arc<dyn ClassifierApiProvider>) {
    CLASSIFIER_APIS
        .write()
        .unwrap()
        .insert(provider.api().to_string(), provider);
}

pub fn unregister_classifier_api(api: &str) {
    CLASSIFIER_APIS.write().unwrap().remove(api);
}

pub fn clear_classifier_apis() {
    CLASSIFIER_APIS.write().unwrap().clear();
}

pub fn register_classifier_model(model: ClassifierModel) {
    CLASSIFIER_MODELS
        .write()
        .unwrap()
        .insert(key(&model.provider, &model.id), model);
}

pub fn register_builtin_classifier_models() {
    for model in crate::classifier_models_generated::builtin_classifier_models() {
        register_classifier_model(model);
    }
}

pub fn clear_classifier_models() {
    CLASSIFIER_MODELS.write().unwrap().clear();
}

pub fn get_classifier_model(provider: &str, id: &str) -> Option<ClassifierModel> {
    ensure_builtins();
    CLASSIFIER_MODELS
        .read()
        .unwrap()
        .get(&key(provider, id))
        .cloned()
}

pub fn list_classifier_models(provider: Option<&str>) -> Vec<ClassifierModel> {
    ensure_builtins();
    let mut models = CLASSIFIER_MODELS
        .read()
        .unwrap()
        .values()
        .filter(|model| provider.is_none_or(|value| model.provider == value))
        .cloned()
        .collect::<Vec<_>>();
    models.sort_by(|left, right| {
        left.provider
            .cmp(&right.provider)
            .then(left.id.cmp(&right.id))
    });
    models
}

pub fn list_classifier_providers() -> Vec<String> {
    let mut providers = list_classifier_models(None)
        .into_iter()
        .map(|model| model.provider)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    providers.sort();
    providers
}

pub async fn classify(
    model: &ClassifierModel,
    context: &ClassifierContext,
    options: &ClassifierOptions,
) -> ClassifierResult {
    ensure_builtins();
    if model.model_type != crate::types::ModelType::Classifier {
        return error_result(model, "model is not a classifier model", false);
    }
    if !context.images.is_empty() && !model.input.iter().any(|input| input == "image") {
        return error_result(
            model,
            format!(
                "Classifier model {}/{} does not support image input",
                model.provider, model.id
            ),
            false,
        );
    }
    let provider = CLASSIFIER_APIS.read().unwrap().get(&model.api).cloned();
    match provider {
        Some(provider) => provider.classify(model, context, options).await,
        None => error_result(
            model,
            format!("No classifier provider registered for api: {}", model.api),
            false,
        ),
    }
}

pub(crate) fn error_result(
    model: &ClassifierModel,
    error: impl ToString,
    aborted: bool,
) -> ClassifierResult {
    ClassifierResult {
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        answers: indexmap::IndexMap::new(),
        usage: None,
        stop_reason: if aborted {
            ClassifierStopReason::Aborted
        } else {
            ClassifierStopReason::Error
        },
        error_message: Some(error.to_string()),
        timestamp: crate::utils::now_millis(),
    }
}

struct TypeSafeProvider;
impl ClassifierApiProvider for TypeSafeProvider {
    fn api(&self) -> &str {
        crate::types::api::TYPESAFE_SYSTEM_ONE
    }
    fn classify<'a>(
        &'a self,
        model: &'a ClassifierModel,
        context: &'a ClassifierContext,
        options: &'a ClassifierOptions,
    ) -> Pin<Box<dyn Future<Output = ClassifierResult> + Send + 'a>> {
        Box::pin(system_one::classify_typesafe(model, context, options))
    }
}

struct CloudflareProvider;
impl ClassifierApiProvider for CloudflareProvider {
    fn api(&self) -> &str {
        crate::types::api::CLOUDFLARE_WORKERS_AI_SYSTEM_ONE
    }
    fn classify<'a>(
        &'a self,
        model: &'a ClassifierModel,
        context: &'a ClassifierContext,
        options: &'a ClassifierOptions,
    ) -> Pin<Box<dyn Future<Output = ClassifierResult> + Send + 'a>> {
        Box::pin(system_one::classify_cloudflare(model, context, options))
    }
}

struct LlamaCppProvider;
impl ClassifierApiProvider for LlamaCppProvider {
    fn api(&self) -> &str {
        crate::types::api::LLAMA_CPP_CLASSIFY
    }
    fn classify<'a>(
        &'a self,
        model: &'a ClassifierModel,
        context: &'a ClassifierContext,
        options: &'a ClassifierOptions,
    ) -> Pin<Box<dyn Future<Output = ClassifierResult> + Send + 'a>> {
        Box::pin(llama_cpp::classify_llama_cpp(model, context, options))
    }
}

struct OpenAIDecisionsProvider;
impl ClassifierApiProvider for OpenAIDecisionsProvider {
    fn api(&self) -> &str {
        crate::types::api::OPENAI_DECISIONS
    }
    fn classify<'a>(
        &'a self,
        model: &'a ClassifierModel,
        context: &'a ClassifierContext,
        options: &'a ClassifierOptions,
    ) -> Pin<Box<dyn Future<Output = ClassifierResult> + Send + 'a>> {
        Box::pin(openai_decisions::classify_openai_decisions(
            model, context, options,
        ))
    }
}

pub fn register_builtin_classifier_providers() {
    register_classifier_api(Arc::new(OpenAIDecisionsProvider));
    register_classifier_api(Arc::new(TypeSafeProvider));
    register_classifier_api(Arc::new(CloudflareProvider));
    register_classifier_api(Arc::new(LlamaCppProvider));
}
