//! Deterministic counterpart for upstream `test/model-data-validation.test.ts`.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rs-ai-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn hex_sha256(text: &str) -> String {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(text.as_bytes()))
    }

    fn make_model(model_id: &str, provider: &str, model_api: &str) -> serde_json::Value {
        serde_json::json!({
            "id":model_id,
            "name":"Model A",
            "api":model_api,
            "provider":provider,
            "baseUrl":"https://example.test/v1",
            "reasoning":false,
            "input":["text"],
            "cost":{"input":1,"output":2,"cacheRead":0,"cacheWrite":0},
            "contextWindow":1000,
            "maxTokens":100
        })
    }

    struct FixtureSpec<'a> {
        api_group: &'a str,
        model_api: &'a str,
        duplicate: bool,
        schema_version: u32,
        generated_at: &'a str,
        model_id: &'a str,
        model_provider: &'a str,
    }

    impl Default for FixtureSpec<'_> {
        fn default() -> Self {
            Self {
                api_group: "openai-completions",
                model_api: "openai-completions",
                duplicate: false,
                schema_version: 3,
                generated_at: "2026-07-23T10:00:00.000Z",
                model_id: "model-a",
                model_provider: "test-provider",
            }
        }
    }

    fn write_fixture(root: &Path, spec: FixtureSpec<'_>) {
        let data_dir = root.join("data");
        fs::create_dir_all(&data_dir).unwrap();
        let content_value = if spec.duplicate {
            serde_json::json!({
                "openai-completions": {"model-a": make_model("model-a", "test-provider", "openai-completions")},
                "anthropic-messages": {"model-a": make_model("model-a", "test-provider", "anthropic-messages")},
            })
        } else {
            serde_json::json!({spec.api_group: {"model-a": make_model(spec.model_id, spec.model_provider, spec.model_api)}})
        };
        let content = format!("{}\n", serde_json::to_string(&content_value).unwrap());
        fs::write(data_dir.join("test-provider.json"), &content).unwrap();
        let hash = hex_sha256(&content);
        let structure_json = format!(
            "{{\"test-provider\":{{\"model-a\":\"{}\"}}}}",
            spec.model_api
        );
        let structure_hash = hex_sha256(&structure_json);
        let manifest = serde_json::json!({
            "schemaVersion": spec.schema_version,
            "generatedAt": spec.generated_at,
            "structureHash": structure_hash,
            "files": {"test-provider.json": hash},
        });
        fs::write(
            data_dir.join(".manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn run_validator(root: &Path) -> Result<String, String> {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/validate_release_model_data.py")
            .arg(root.join("data"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[test]
    fn validates_api_grouped_model_data_and_rejects_failure_modes() {
        let root = temp_root("model-data-ok");
        write_fixture(&root, FixtureSpec::default());
        assert!(run_validator(&root).unwrap().contains("\"models\": 1"));
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-wrong-api");
        write_fixture(
            &root,
            FixtureSpec {
                api_group: "anthropic-messages",
                ..Default::default()
            },
        );
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("grouped under API")
        );
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-wrong-id");
        write_fixture(
            &root,
            FixtureSpec {
                model_id: "wrong-id",
                ..Default::default()
            },
        );
        assert!(run_validator(&root).unwrap_err().contains("has id"));
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-wrong-provider");
        write_fixture(
            &root,
            FixtureSpec {
                model_provider: "wrong-provider",
                ..Default::default()
            },
        );
        assert!(run_validator(&root).unwrap_err().contains("has provider"));
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-duplicate");
        write_fixture(
            &root,
            FixtureSpec {
                duplicate: true,
                ..Default::default()
            },
        );
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("more than one API group")
        );
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-stale-hash");
        write_fixture(&root, FixtureSpec::default());
        fs::write(root.join("data/test-provider.json"), "{}\n").unwrap();
        assert!(run_validator(&root).unwrap_err().contains("manifest hash"));
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-missing-dir");
        fs::create_dir_all(root.join("data")).unwrap();
        fs::remove_dir_all(root.join("data")).unwrap();
        assert!(run_validator(&root).unwrap_err().contains("does not exist"));
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-schema");
        write_fixture(
            &root,
            FixtureSpec {
                schema_version: 4,
                ..Default::default()
            },
        );
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("model data schema")
        );
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-stale-stamp");
        write_fixture(&root, FixtureSpec::default());
        let manifest_path = root.join("data/.manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["structureHash"] = serde_json::json!("stale");
        fs::write(&manifest_path, manifest.to_string()).unwrap();
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("generation stamp")
        );
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-timestamp");
        write_fixture(
            &root,
            FixtureSpec {
                generated_at: "invalid",
                ..Default::default()
            },
        );
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("generation timestamp")
        );
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-missing-shard");
        write_fixture(&root, FixtureSpec::default());
        fs::remove_file(root.join("data/test-provider.json")).unwrap();
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("missing provider shard")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
