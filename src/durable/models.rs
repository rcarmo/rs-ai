//! Host model access passed unchanged to native durable tool executions.
//! This seam does not add hook/task registries or account nested-call usage.
use crate::classifiers::ClassifierOptions;
use crate::types::{
    ClassifierContext, ClassifierModel, ClassifierResult, Context, Message, Model, StreamOptions,
};
use std::future::Future;
use std::pin::Pin;

pub type CompletionFuture<'a> = Pin<Box<dyn Future<Output = Result<Message, String>> + Send + 'a>>;
pub type ClassificationFuture<'a> = Pin<Box<dyn Future<Output = ClassifierResult> + Send + 'a>>;

/// An application can supply its own credential-scoped model service. The same
/// shared object is handed to every normal or recovered tool execution.
pub trait DurableModels: Send + Sync + 'static {
    fn get_model(&self, provider: &str, id: &str) -> Option<Model>;
    fn complete<'a>(
        &'a self,
        model: &'a Model,
        context: &'a Context,
        options: &'a StreamOptions,
    ) -> CompletionFuture<'a>;
    fn classify<'a>(
        &'a self,
        model: &'a ClassifierModel,
        context: &'a ClassifierContext,
        options: &'a ClassifierOptions,
    ) -> ClassificationFuture<'a>;
}

/// Default service uses the existing native public registries and transports.
#[derive(Default)]
pub struct RegistryModels;
impl DurableModels for RegistryModels {
    fn get_model(&self, provider: &str, id: &str) -> Option<Model> {
        crate::registry::get_model(provider, id)
    }
    fn complete<'a>(
        &'a self,
        model: &'a Model,
        context: &'a Context,
        options: &'a StreamOptions,
    ) -> CompletionFuture<'a> {
        Box::pin(async move {
            crate::registry::complete(model, context, options)
                .await
                .map_err(|error| error.to_string())
        })
    }
    fn classify<'a>(
        &'a self,
        model: &'a ClassifierModel,
        context: &'a ClassifierContext,
        options: &'a ClassifierOptions,
    ) -> ClassificationFuture<'a> {
        Box::pin(crate::classifiers::classify(model, context, options))
    }
}
