//! Shared Azure endpoint/deployment resolution for Responses and Chat Completions.
use crate::types::{Model, StreamOptions};

fn env(options: &StreamOptions, name: &str) -> Option<String> {
    if let Some(value) = options.env.as_ref().and_then(|env| env.get(name)) {
        return Some(value.clone());
    }
    std::env::var(name).ok()
}

pub(crate) fn config(model: &Model, options: &StreamOptions) -> Result<(String, String), String> {
    let base = options
        .azure_base_url
        .as_deref()
        .filter(|base| !base.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| env(options, "AZURE_OPENAI_BASE_URL"));
    let resource = options
        .azure_resource_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| env(options, "AZURE_OPENAI_RESOURCE_NAME"));
    let base = super::responses::resolve_azure_base_url_from(base.as_deref(), resource.as_deref(), &model.base_url)?
        .ok_or_else(|| "Azure OpenAI base URL is required. Set AZURE_OPENAI_BASE_URL or AZURE_OPENAI_RESOURCE_NAME, or pass azureBaseUrl, azureResourceName, or model.baseUrl.".to_owned())?;
    let version = options
        .azure_api_version
        .as_deref()
        .filter(|version| !version.is_empty())
        .map(str::to_owned)
        .or_else(|| env(options, "AZURE_OPENAI_API_VERSION").filter(|version| !version.is_empty()))
        .unwrap_or_else(|| "v1".into());
    Ok((base, version))
}

pub(crate) fn deployment(model: &Model, options: &StreamOptions) -> String {
    if let Some(name) = options
        .azure_deployment_name
        .as_deref()
        .filter(|name| !name.is_empty())
    {
        return name.to_owned();
    }
    super::responses::resolve_azure_deployment_from_map(
        env(options, "AZURE_OPENAI_DEPLOYMENT_NAME_MAP").as_deref(),
        &model.id,
    )
}

pub(crate) fn endpoint(
    base: &str,
    suffix: &str,
    api_version: Option<&str>,
) -> Result<String, String> {
    let mut url =
        url::Url::parse(base).map_err(|_| format!("Invalid Azure OpenAI base URL: {base}"))?;
    url.set_path(&format!("{}/{}", url.path().trim_end_matches('/'), suffix));
    if let Some(version) = api_version {
        url.query_pairs_mut().append_pair("api-version", version);
    }
    Ok(url.to_string())
}
