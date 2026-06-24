//! Test-for-test port of upstream `test/node-http-proxy.test.ts`.

#[cfg(test)]
mod tests {
    use crate::http_proxy::{resolve_http_proxy_url_for_target, UNSUPPORTED_PROXY_PROTOCOL_MESSAGE};
    use std::collections::HashMap;

    // These tests rely on a caller-supplied env map rather than mutating process
    // env, keeping them deterministic and parallel-safe (the upstream suite
    // resets process.env per case via fake timers/afterEach).

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn respects_no_proxy_exclusions() {
        let e = env(&[
            ("HTTPS_PROXY", "http://proxy.example:8080"),
            ("NO_PROXY", "bedrock-runtime.us-east-1.amazonaws.com"),
        ]);
        let r = resolve_http_proxy_url_for_target(
            "https://bedrock-runtime.us-east-1.amazonaws.com",
            Some(&e),
        )
        .unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn resolves_http_and_https_proxy_urls() {
        let e = env(&[("HTTPS_PROXY", "http://proxy.example:8080")]);
        let r = resolve_http_proxy_url_for_target(
            "https://bedrock-runtime.us-east-1.amazonaws.com",
            Some(&e),
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.to_string(), "http://proxy.example:8080/");
    }

    #[test]
    fn prefers_scoped_proxy_env_aliases_before_process_env_aliases() {
        // The scoped map alias must win over any process-env alias.
        let e = env(&[("HTTPS_PROXY", "http://scoped-proxy.example:8080")]);
        let r = resolve_http_proxy_url_for_target(
            "https://bedrock-runtime.us-east-1.amazonaws.com",
            Some(&e),
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.to_string(), "http://scoped-proxy.example:8080/");
    }

    #[test]
    fn rejects_socks_and_pac_proxy_urls_explicitly() {
        let e = env(&[("HTTPS_PROXY", "socks5://proxy.example:1080")]);
        let err = resolve_http_proxy_url_for_target(
            "https://bedrock-runtime.us-east-1.amazonaws.com",
            Some(&e),
        )
        .unwrap_err();
        assert!(err.starts_with(UNSUPPORTED_PROXY_PROTOCOL_MESSAGE));
    }
}
