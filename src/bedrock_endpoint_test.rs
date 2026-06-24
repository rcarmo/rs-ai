//! Adaptation of @go-ai local bedrock endpoint-resolution tests
//! (`inference/provider/bedrock/bedrock_test.go`: `TestExtractRegionFromURL`,
//! `TestShouldUseExplicitBedrockEndpoint`) into idiomatic Rust.
//!
//! These guard the standard-vs-custom Bedrock endpoint logic that drives region
//! resolution. rs-ai's `bedrock_use_explicit_endpoint` reads region/profile from
//! the environment (vs go-ai's pure params), so the env-dependent cases set and
//! clear the relevant vars within the test, matching rs-ai's existing env-test
//! convention (`src/env_test.rs`).

#[cfg(test)]
mod tests {
    use crate::provider::bedrock::{
        bedrock_arn_region, bedrock_standard_endpoint_region, bedrock_use_explicit_endpoint, resolve_bedrock_region,
    };
    use crate::registry::get_model;
    use crate::types::{Model, ModelCost};
    use std::sync::Mutex;

    // Serialize AWS-env-mutating tests in this module.
    static AWS_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn bedrock_model(base_url: &str, id: &str) -> Model {
        Model {
            id: id.into(),
            name: "Claude".into(),
            api: "bedrock-converse-stream".into(),
            provider: "amazon-bedrock".into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    /// go-ai `TestExtractRegionFromURL` — `extractRegionFromURL` maps to rs-ai's
    /// `bedrock_standard_endpoint_region` (None == go-ai's "").
    #[test]
    fn extract_region_from_url() {
        let cases: &[(&str, Option<&str>)] = &[
            ("https://bedrock-runtime.us-east-1.amazonaws.com", Some("us-east-1")),
            ("https://bedrock-runtime.eu-west-1.amazonaws.com", Some("eu-west-1")),
            ("https://bedrock-runtime-fips.us-gov-west-1.amazonaws.com", Some("us-gov-west-1")),
            ("https://bedrock-runtime.cn-north-1.amazonaws.com.cn", Some("cn-north-1")),
            ("https://example.com", None),
            ("", None),
        ];
        for (url, want) in cases {
            assert_eq!(
                bedrock_standard_endpoint_region(url).as_deref(),
                *want,
                "bedrock_standard_endpoint_region({url:?})"
            );
        }
    }

    /// Bonus pure-region coverage: inference-profile ARN ids carry the region in
    /// the 4th `:`-delimited field.
    #[test]
    fn extract_region_from_arn_model_id() {
        assert_eq!(
            bedrock_arn_region("arn:aws:bedrock:eu-west-3:123:inference-profile/x").as_deref(),
            Some("eu-west-3")
        );
        assert_eq!(bedrock_arn_region("anthropic.claude-3-5-sonnet"), None);
    }

    /// go-ai `TestShouldUseExplicitBedrockEndpoint`. rs-ai reads region/profile
    /// from the environment, so we control `AWS_REGION`/`AWS_DEFAULT_REGION`/
    /// `AWS_PROFILE` for the standard-endpoint cases.
    #[test]
    fn should_use_explicit_bedrock_endpoint() {
        let _g = AWS_ENV_LOCK.lock().unwrap();
        let standard = bedrock_model("https://bedrock-runtime.us-east-1.amazonaws.com", "anthropic.claude");
        let custom = bedrock_model("https://custom-bedrock-proxy.example.com", "anthropic.claude");

        // Custom endpoints are always pinned, regardless of env.
        assert!(bedrock_use_explicit_endpoint(&custom), "custom endpoints must stay explicit");

        // Standard endpoint with no region/profile configured -> pinned.
        unsafe {
            std::env::remove_var("AWS_REGION");
            std::env::remove_var("AWS_DEFAULT_REGION");
            std::env::remove_var("AWS_PROFILE");
        }
        assert!(
            bedrock_use_explicit_endpoint(&standard),
            "standard endpoint must be pinned when no region/profile is configured"
        );

        // Standard endpoint with a region configured -> not pinned (SDK resolves it).
        unsafe { std::env::set_var("AWS_REGION", "eu-west-1"); }
        let pinned_with_region = bedrock_use_explicit_endpoint(&standard);
        unsafe { std::env::remove_var("AWS_REGION"); }
        assert!(!pinned_with_region, "standard endpoint must not be pinned when a region is configured");
    }

    // --- upstream bedrock-endpoint-resolution region cases ---

    fn clear_aws_env() {
        unsafe {
            std::env::remove_var("AWS_REGION");
            std::env::remove_var("AWS_DEFAULT_REGION");
            std::env::remove_var("AWS_PROFILE");
        }
    }

    #[test]
    fn assigns_eu_central_1_url_to_builtin_eu_inference_profiles() {
        let m = get_model("amazon-bedrock", "eu.anthropic.claude-sonnet-4-5-20250929-v1:0").unwrap();
        assert_eq!(m.base_url, "https://bedrock-runtime.eu-central-1.amazonaws.com");
    }

    #[test]
    fn resolves_region_serialized_cases() {
        // env-coupled: serialize via the shared lock and a clean slate.
        let _g = AWS_ENV_LOCK.lock().unwrap();
        clear_aws_env();

        // EU endpoint with no region/profile -> region derived from the endpoint.
        let eu = get_model("amazon-bedrock", "eu.anthropic.claude-sonnet-4-5-20250929-v1:0").unwrap();
        assert_eq!(resolve_bedrock_region(&eu).as_deref(), Some("eu-central-1"));

        // Configured AWS_REGION wins for a standard endpoint.
        let us = get_model("amazon-bedrock", "us.anthropic.claude-opus-4-8").unwrap();
        unsafe { std::env::set_var("AWS_REGION", "us-east-2"); }
        assert_eq!(resolve_bedrock_region(&us).as_deref(), Some("us-east-2"));
        clear_aws_env();

        // An inference-profile ARN region wins over AWS_REGION.
        let mut arn = us.clone();
        arn.id = "arn:aws:bedrock:us-west-2:123456789012:application-inference-profile/abc123".into();
        unsafe { std::env::set_var("AWS_REGION", "us-east-1"); }
        assert_eq!(resolve_bedrock_region(&arn).as_deref(), Some("us-west-2"));

        // GovCloud ARN region.
        let mut gov = us.clone();
        gov.id = "arn:aws-us-gov:bedrock:us-gov-west-1:123456789012:application-inference-profile/abc123".into();
        assert_eq!(resolve_bedrock_region(&gov).as_deref(), Some("us-gov-west-1"));
        clear_aws_env();
    }
}
