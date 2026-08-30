//! Provider implementations and registration helpers.

use std::sync::Arc;

use crate::events::Event;
use crate::registry::{self, ApiProvider};
use crate::types::{Context, Model, StreamOptions};
use tokio_stream::Stream;

pub mod anthropic;
#[cfg(feature = "bedrock")]
pub mod bedrock;
pub mod codex;
pub mod faux;
pub mod google;
pub mod mistral;
pub mod openai;
pub mod pi_messages;
pub mod responses;

struct OpenAIProvider;
impl ApiProvider for OpenAIProvider {
    fn api(&self) -> &str {
        "openai-completions"
    }
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        openai::stream_openai(model, context, opts)
    }
}

struct OpenAIResponsesProvider;
impl ApiProvider for OpenAIResponsesProvider {
    fn api(&self) -> &str {
        "openai-responses"
    }
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        responses::stream_responses(model, context, opts)
    }
}

struct AzureOpenAIResponsesProvider;
impl ApiProvider for AzureOpenAIResponsesProvider {
    fn api(&self) -> &str {
        "azure-openai-responses"
    }
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        responses::stream_azure_responses(model, context, opts)
    }
}

struct AnthropicProvider;
impl ApiProvider for AnthropicProvider {
    fn api(&self) -> &str {
        "anthropic-messages"
    }
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        anthropic::stream_anthropic(model, context, opts)
    }
}

struct GoogleProvider;
impl ApiProvider for GoogleProvider {
    fn api(&self) -> &str {
        "google-generative-ai"
    }
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        google::stream_google(model, context, opts)
    }
}

struct GoogleVertexProvider;
impl ApiProvider for GoogleVertexProvider {
    fn api(&self) -> &str {
        "google-vertex"
    }
    // Vertex AI uses the same @google/genai wire format as Gemini, so the shared
    // `stream_google` decoder applies. The request path differs (project/location-scoped
    // endpoint, `{location}` host substitution); `build_stream_url` handles that and
    // resolves project/location from StreamOptions or GOOGLE_CLOUD_* env vars.
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        google::stream_google(model, context, opts)
    }
}

struct MistralProvider;
impl ApiProvider for MistralProvider {
    fn api(&self) -> &str {
        "mistral-conversations"
    }
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        mistral::stream_mistral(model, context, opts)
    }
}

#[cfg(feature = "bedrock")]
struct BedrockProvider;
#[cfg(feature = "bedrock")]
impl ApiProvider for BedrockProvider {
    fn api(&self) -> &str {
        "bedrock-converse-stream"
    }
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        bedrock::stream_bedrock(model, context, opts)
    }
}

struct PiMessagesProvider;
impl ApiProvider for PiMessagesProvider {
    fn api(&self) -> &str {
        "pi-messages"
    }
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        pi_messages::stream_pi_messages(model, context, opts)
    }
}

struct CodexProvider;
impl ApiProvider for CodexProvider {
    fn api(&self) -> &str {
        "openai-codex-responses"
    }
    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Event> + Send + 'a>> {
        codex::stream_codex(model, context, opts)
    }
}

/// Register all built-in provider implementations.
pub fn register_builtin_providers() {
    registry::register_api(Arc::new(OpenAIProvider));
    registry::register_api(Arc::new(OpenAIResponsesProvider));
    registry::register_api(Arc::new(AzureOpenAIResponsesProvider));
    registry::register_api(Arc::new(AnthropicProvider));
    registry::register_api(Arc::new(GoogleProvider));
    registry::register_api(Arc::new(GoogleVertexProvider));
    registry::register_api(Arc::new(MistralProvider));
    #[cfg(feature = "bedrock")]
    registry::register_api(Arc::new(BedrockProvider));
    registry::register_api(Arc::new(PiMessagesProvider));
    registry::register_api(Arc::new(CodexProvider));
}
