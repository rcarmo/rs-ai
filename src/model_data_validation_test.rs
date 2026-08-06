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

    fn make_model(model_api: &str) -> serde_json::Value {
        serde_json::json!({
            "id":"model-a",
            "name":"Model A",
            "api":model_api,
            "provider":"test-provider",
            "baseUrl":"https://example.test/v1",
            "reasoning":false,
            "input":["text"],
            "cost":{"input":1,"output":2,"cacheRead":0,"cacheWrite":0},
            "contextWindow":1000,
            "maxTokens":100
        })
    }

    fn write_fixture(
        root: &Path,
        api_group: &str,
        model_api: &str,
        duplicate: bool,
        schema_version: u32,
        generated_at: &str,
    ) {
        let data_dir = root.join("data");
        fs::create_dir_all(&data_dir).unwrap();
        let content_value = if duplicate {
            serde_json::json!({
                "openai-completions": {"model-a": make_model("openai-completions")},
                "anthropic-messages": {"model-a": make_model("anthropic-messages")},
            })
        } else {
            serde_json::json!({api_group: {"model-a": make_model(model_api)}})
        };
        let content = format!("{}\n", serde_json::to_string(&content_value).unwrap());
        fs::write(data_dir.join("test-provider.json"), &content).unwrap();
        let hash = hex_sha256(&content);
        let structure_json = format!("{{\"test-provider\":{{\"model-a\":\"{model_api}\"}}}}");
        let structure_hash = hex_sha256(&structure_json);
        let manifest = serde_json::json!({
            "schemaVersion": schema_version,
            "generatedAt": generated_at,
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
        write_fixture(
            &root,
            "openai-completions",
            "openai-completions",
            false,
            3,
            "2026-07-23T10:00:00.000Z",
        );
        assert!(run_validator(&root).unwrap().contains("\"models\": 1"));
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-wrong-api");
        write_fixture(
            &root,
            "anthropic-messages",
            "openai-completions",
            false,
            3,
            "2026-07-23T10:00:00.000Z",
        );
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("grouped under API")
        );
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-duplicate");
        write_fixture(
            &root,
            "openai-completions",
            "openai-completions",
            true,
            3,
            "2026-07-23T10:00:00.000Z",
        );
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("more than one API group")
        );
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-stale-hash");
        write_fixture(
            &root,
            "openai-completions",
            "openai-completions",
            false,
            3,
            "2026-07-23T10:00:00.000Z",
        );
        fs::write(root.join("data/test-provider.json"), "{}\n").unwrap();
        assert!(run_validator(&root).unwrap_err().contains("manifest hash"));
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-schema");
        write_fixture(
            &root,
            "openai-completions",
            "openai-completions",
            false,
            4,
            "2026-07-23T10:00:00.000Z",
        );
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("model data schema")
        );
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-timestamp");
        write_fixture(
            &root,
            "openai-completions",
            "openai-completions",
            false,
            3,
            "invalid",
        );
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("generation timestamp")
        );
        fs::remove_dir_all(root).unwrap();

        let root = temp_root("model-data-missing-shard");
        write_fixture(
            &root,
            "openai-completions",
            "openai-completions",
            false,
            3,
            "2026-07-23T10:00:00.000Z",
        );
        fs::remove_file(root.join("data/test-provider.json")).unwrap();
        assert!(
            run_validator(&root)
                .unwrap_err()
                .contains("missing provider shard")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
