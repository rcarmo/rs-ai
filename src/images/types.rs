//! Image generation types.

use crate::types::{ModelCost, StopReason, Usage};
use serde::{Deserialize, Serialize};

/// Image API identifier.
pub type ImageApi = String;
/// Backwards-compatible alias retained for the pre-v0.99 plural spelling.
pub type ImagesApi = ImageApi;

/// Image provider identifier.
pub type ImageProvider = String;
/// Backwards-compatible alias retained for the pre-v0.99 plural spelling.
pub type ImagesProvider = ImageProvider;

/// Input content for image generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ImageInput {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
}

/// Image generation context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesContext {
    pub input: Vec<ImageInput>,
}

/// Output item from image generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ImageOutput {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
}

/// Image model definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageModel {
    #[serde(rename = "type")]
    pub model_type: crate::types::ModelType,
    pub id: String,
    pub name: String,
    pub api: ImageApi,
    pub provider: ImageProvider,
    pub base_url: String,
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_limits: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Vec<String>,
    pub cost: ModelCost,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

/// Backwards-compatible alias retained for existing rs-ai callers.
pub type ImagesModel = ImageModel;

/// Result of an image generation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantImages {
    pub api: ImagesApi,
    pub provider: ImagesProvider,
    pub model: String,
    pub output: Vec<ImageOutput>,
    pub stop_reason: StopReason,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}
