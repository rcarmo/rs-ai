//! Test-for-test port of the portable substance of upstream
//! `test/bedrock-custom-headers.test.ts` (`@earendil-works/pi-ai` v0.80.2), plus
//! the @go-ai `TestBedrockCustomHeaderReservation` adaptation.
//!
//! The upstream cases assert the AWS-SDK `build`-step middleware mechanism (JS
//! Smithy middleware registration/invocation), which rs-ai implements differently
//! (a `mutate_request` customization). The portable, mechanism-independent
//! substance is the reserved-header rule: caller headers override existing ones,
//! except reserved SigV4/auth headers (`x-amz-*`, `authorization`, `host`,
//! case-insensitive), which are skipped.

#[cfg(test)]
mod tests {
    use crate::provider::bedrock::is_reserved_bedrock_header;
    use std::collections::BTreeMap;

    #[test]
    fn reserves_sigv4_and_auth_headers_case_insensitively() {
        for key in ["authorization", "Authorization", "host", "Host", "x-amz-date", "X-Amz-Security-Token", "x-amz-content-sha256"] {
            assert!(is_reserved_bedrock_header(key), "{key} must be reserved");
        }
        for key in ["x-trace-id", "anthropic-beta", "x-allowed", "x-custom"] {
            assert!(!is_reserved_bedrock_header(key), "{key} must be allowed");
        }
    }

    /// Mirrors upstream VC2: applying caller headers skips reserved keys
    /// (case-insensitively) and keeps the pre-existing signed values, while
    /// allowed headers are added. Modelled as a case-insensitive header map.
    #[test]
    fn skips_reserved_headers_while_applying_allowed_ones() {
        // Pre-existing (signed) request headers.
        let mut request: BTreeMap<String, String> = BTreeMap::from([
            ("authorization".to_string(), "real-auth".to_string()),
            ("x-amz-date".to_string(), "real-date".to_string()),
            ("host".to_string(), "real-host".to_string()),
        ]);
        // Caller headers (mixed case; reserved + allowed).
        let caller: Vec<(&str, &str)> = vec![
            ("authorization", "evil"), ("x-amz-date", "evil"), ("x-allowed", "ok"),
            ("Authorization", "evil2"), ("X-Amz-Date", "evil2"), ("HOST", "evil3"),
        ];
        for (k, v) in caller {
            if !is_reserved_bedrock_header(k) {
                // Allowed headers override using their canonical lowercase key
                // (HTTP header maps are case-insensitive).
                request.insert(k.to_ascii_lowercase(), v.to_string());
            }
        }
        assert_eq!(request.get("authorization").map(String::as_str), Some("real-auth"));
        assert_eq!(request.get("x-amz-date").map(String::as_str), Some("real-date"));
        assert_eq!(request.get("host").map(String::as_str), Some("real-host"));
        assert_eq!(request.get("x-allowed").map(String::as_str), Some("ok"));
        // No mixed-case reserved leak.
        assert!(request.get("Authorization").is_none());
        assert!(request.get("X-Amz-Date").is_none());
        assert!(request.get("HOST").is_none());
        let keys: Vec<&String> = request.keys().collect();
        assert_eq!(keys, vec!["authorization", "host", "x-allowed", "x-amz-date"]);
    }
}
