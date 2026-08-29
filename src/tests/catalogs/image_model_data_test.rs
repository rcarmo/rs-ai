//! Deterministic counterpart for upstream `test/image-model-data.test.ts`.
//!
//! Exercises the production parser helper in `scripts/generate_image_models.py`.

#[cfg(test)]
mod tests {
    use std::process::Command;

    fn run_parser(payload: serde_json::Value, strict: bool) -> Result<serde_json::Value, String> {
        let code = r#"
import importlib.util, json, pathlib, sys
spec = importlib.util.spec_from_file_location('generate_image_models', pathlib.Path('scripts/generate_image_models.py'))
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
try:
    result = mod.parse_openrouter_image_models(json.loads(sys.argv[1]), sys.argv[2] == 'true')
    print(json.dumps(result, sort_keys=True))
except Exception as e:
    print(str(e), file=sys.stderr)
    raise SystemExit(1)
"#;
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("-c")
            .arg(code)
            .arg(payload.to_string())
            .arg(if strict { "true" } else { "false" })
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        if output.status.success() {
            Ok(serde_json::from_slice(&output.stdout).unwrap())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
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
            assert!(
                run_parser(payload, true)
                    .unwrap_err()
                    .contains("missing or empty image model list")
            );
        }
    }

    #[test]
    fn rejects_strict_catalog_with_no_usable_image_models() {
        let mut model = valid_image_model();
        model["architecture"] =
            serde_json::json!({"input_modalities":["text"],"output_modalities":["text"]});
        assert!(
            run_parser(serde_json::json!({"data":[model]}), true)
                .unwrap_err()
                .contains("no usable image models")
        );
    }

    #[test]
    fn parses_non_empty_image_model_catalog() {
        let parsed = run_parser(serde_json::json!({"data":[valid_image_model()]}), true).unwrap();
        assert_eq!(parsed[0]["id"], "example/image-model");
        assert_eq!(parsed[0]["input"], serde_json::json!(["text", "image"]));
        assert_eq!(parsed[0]["output"], serde_json::json!(["image"]));
    }
}
