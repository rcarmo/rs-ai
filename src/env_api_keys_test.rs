//! Test-for-test port of upstream `test/env-api-keys.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2): provider env-key resolution.
//!
//! rs-ai exposes `get_env_api_key` (the resolution); the upstream `findEnvKeys`
//! listing is asserted through resolution behaviour.

#[cfg(test)]
mod tests {
    use crate::env::get_env_api_key;
    use std::sync::Mutex;

    // These tests mutate process-global env vars; serialize them against each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear() {
        unsafe {
            std::env::remove_var("COPILOT_GITHUB_TOKEN");
            std::env::remove_var("GH_TOKEN");
            std::env::remove_var("GITHUB_TOKEN");
            std::env::remove_var("ZAI_CODING_CN_API_KEY");
        }
    }

    #[test]
    fn does_not_treat_generic_github_tokens_as_github_copilot_credentials() {
        let _g = ENV_LOCK.lock().unwrap();
        clear();
        unsafe {
            std::env::set_var("GH_TOKEN", "gh-token");
            std::env::set_var("GITHUB_TOKEN", "github-token");
        }
        let got = get_env_api_key("github-copilot");
        clear();
        assert!(got.is_none(), "generic GitHub tokens must not resolve github-copilot");
    }

    #[test]
    fn resolves_github_copilot_credentials_from_copilot_github_token() {
        let _g = ENV_LOCK.lock().unwrap();
        clear();
        unsafe {
            std::env::set_var("COPILOT_GITHUB_TOKEN", "copilot-token");
            std::env::set_var("GH_TOKEN", "gh-token");
            std::env::set_var("GITHUB_TOKEN", "github-token");
        }
        let got = get_env_api_key("github-copilot");
        clear();
        assert_eq!(got.as_deref(), Some("copilot-token"));
    }

    #[test]
    fn resolves_zai_china_coding_plan_from_zai_coding_cn_api_key() {
        let _g = ENV_LOCK.lock().unwrap();
        clear();
        unsafe { std::env::set_var("ZAI_CODING_CN_API_KEY", "zai-coding-cn-token"); }
        let got = get_env_api_key("zai-coding-cn");
        clear();
        assert_eq!(got.as_deref(), Some("zai-coding-cn-token"));
    }
}
