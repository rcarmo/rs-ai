//! Test-for-test port of upstream `test/supports-xhigh.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — `getSupportedThinkingLevels`.

#[cfg(test)]
mod tests {
    use crate::registry::get_model;
    use crate::simple_options::get_supported_thinking_levels;
    use crate::types::Model;

    fn levels(model: &Model) -> Vec<String> {
        get_supported_thinking_levels(model).into_iter()
            .map(|l| serde_json::to_value(l).unwrap().as_str().unwrap().to_string())
            .collect()
    }

    fn m(provider: &str, id: &str) -> Model {
        get_model(provider, id).unwrap_or_else(|| panic!("catalog {provider}/{id}"))
    }

    #[test]
    fn includes_xhigh_for_anthropic_opus_4_6() {
        assert!(levels(&m("anthropic", "claude-opus-4-6")).iter().any(|l| l == "xhigh"));
    }

    #[test]
    fn includes_xhigh_for_anthropic_opus_4_8() {
        assert!(levels(&m("anthropic", "claude-opus-4-8")).iter().any(|l| l == "xhigh"));
    }

    #[test]
    fn includes_xhigh_but_not_off_for_anthropic_fable_5() {
        let l = levels(&m("anthropic", "claude-fable-5"));
        assert!(l.iter().any(|x| x == "xhigh"));
        assert!(!l.iter().any(|x| x == "off"));
    }

    #[test]
    fn does_not_include_xhigh_for_claude_sonnet_4_5() {
        assert!(!levels(&m("anthropic", "claude-sonnet-4-5")).iter().any(|l| l == "xhigh"));
    }

    #[test]
    fn includes_xhigh_for_codex_gpt_5_4_and_5_5() {
        for id in ["gpt-5.4", "gpt-5.5", "gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"] {
            assert!(levels(&m("openai-codex", id)).iter().any(|l| l == "xhigh"), "model {id}");
        }
    }

    #[test]
    fn includes_xhigh_for_openai_gpt_5_6_models() {
        for id in ["gpt-5.6", "gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"] {
            assert!(levels(&m("openai", id)).iter().any(|l| l == "xhigh"), "model {id}");
        }
    }

    #[test]
    fn includes_only_medium_high_xhigh_for_openai_gpt_5_5_pro() {
        assert_eq!(levels(&m("openai", "gpt-5.5-pro")), vec!["medium", "high", "xhigh"]);
    }

    #[test]
    fn includes_only_medium_high_xhigh_for_openrouter_gpt_5_5_pro() {
        assert_eq!(levels(&m("openrouter", "openai/gpt-5.5-pro")), vec!["medium", "high", "xhigh"]);
    }

    #[test]
    fn deepseek_v4_flash_off_high_xhigh_on_deepseek() {
        assert_eq!(levels(&m("deepseek", "deepseek-v4-flash")), vec!["off", "high", "xhigh"]);
    }

    #[test]
    fn deepseek_v4_flash_off_high_xhigh_on_opencode_go() {
        assert_eq!(levels(&m("opencode-go", "deepseek-v4-flash")), vec!["off", "high", "xhigh"]);
    }

    #[test]
    fn opencode_go_kimi_k2_6_off_high() {
        assert_eq!(levels(&m("opencode-go", "kimi-k2.6")), vec!["off", "high"]);
    }

    #[test]
    fn moonshot_kimi_k2_7_code_excludes_off() {
        for provider in ["moonshotai", "moonshotai-cn"] {
            assert_eq!(levels(&m(provider, "kimi-k2.7-code")), vec!["minimal", "low", "medium", "high"]);
        }
    }

    #[test]
    fn opencode_grok_build_only_high() {
        assert_eq!(levels(&m("opencode", "grok-build-0.1")), vec!["high"]);
    }

    #[test]
    fn deepseek_v4_flash_off_high_xhigh_on_openrouter() {
        assert_eq!(levels(&m("openrouter", "deepseek/deepseek-v4-flash")), vec!["off", "high", "xhigh"]);
    }

    #[test]
    fn includes_xhigh_for_openrouter_opus_4_6_completions_api() {
        assert!(levels(&m("openrouter", "anthropic/claude-opus-4.6")).iter().any(|l| l == "xhigh"));
    }

    #[test]
    fn includes_xhigh_but_not_off_for_bedrock_fable_5() {
        let l = levels(&m("amazon-bedrock", "global.anthropic.claude-fable-5"));
        assert!(l.iter().any(|x| x == "xhigh"));
        assert!(!l.iter().any(|x| x == "off"));
    }
}
