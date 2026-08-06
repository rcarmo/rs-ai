//! Deterministic counterpart for upstream `test/image-model-data.test.ts`.

#[cfg(test)]
mod tests {
    fn parse_openrouter_image_models(
        payload: &serde_json::Value,
        strict: bool,
    ) -> Result<Vec<String>, String> {
        let data = payload
            .get("data")
            .and_then(|v| v.as_array())
            .filter(|items| !items.is_empty())
            .ok_or_else(|| "missing or empty image model list".to_string())?;
        let mut out = Vec::new();
        for item in data {
            let input = item
                .pointer("/architecture/input_modalities")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>();
            let output = item
                .pointer("/architecture/output_modalities")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>();
            if output.contains(&"image")
                && (input.contains(&"text") || input.contains(&"image"))
                && let Some(id) = item.get("id").and_then(|v| v.as_str())
            {
                out.push(id.to_string());
            }
        }
        if strict && out.is_empty() {
            return Err("no usable image models".into());
        }
        Ok(out)
    }

    fn valid_image_model() -> serde_json::Value {
        serde_json::json!({
            "id":"example/image-model",
            "name":"Example Image Model",
            "architecture":{"input_modalities":["text","image"],"output_modalities":["image"]},
            "pricing":{"prompt":"0.000001","completion":"0.000002"}
        })
    }

    #[test]
    fn rejects_missing_or_empty_strict_catalog() {
        for payload in [
            serde_json::json!({}),
            serde_json::json!({"data": []}),
            serde_json::json!({"data":"invalid"}),
        ] {
            assert_eq!(
                parse_openrouter_image_models(&payload, true).unwrap_err(),
                "missing or empty image model list"
            );
        }
    }

    #[test]
    fn rejects_strict_catalog_with_no_usable_image_models() {
        let mut model = valid_image_model();
        model["architecture"] =
            serde_json::json!({"input_modalities":["text"],"output_modalities":["text"]});
        assert_eq!(
            parse_openrouter_image_models(&serde_json::json!({"data":[model]}), true).unwrap_err(),
            "no usable image models"
        );
    }

    #[test]
    fn parses_non_empty_image_model_catalog() {
        let parsed =
            parse_openrouter_image_models(&serde_json::json!({"data":[valid_image_model()]}), true)
                .unwrap();
        assert_eq!(parsed, vec!["example/image-model".to_string()]);
    }
}
