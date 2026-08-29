//! Test-for-test adaptation of upstream `test/reasoning-options.test.ts`.
//!
//! Exercises the production generator helper in `scripts/generate_models.py`.

#[cfg(test)]
mod tests {
    use std::process::Command;

    fn run_helper(controls_json: &str) -> serde_json::Value {
        let code = r#"
import importlib.util, json, pathlib, sys
spec = importlib.util.spec_from_file_location('generate_models', pathlib.Path('scripts/generate_models.py'))
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
result = mod.get_effort_thinking_level_map(json.loads(sys.argv[1]))
print(json.dumps(result, sort_keys=True))
"#;
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("-c")
            .arg(code)
            .arg(controls_json)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    #[test]
    fn exposes_only_verified_effort_values_and_none() {
        let got = run_helper(
            r#"[{"type":"toggle"},{"type":"effort","values":["none","low","high","max"]}]"#,
        );
        assert_eq!(
            got,
            serde_json::json!({
                "off":"none",
                "minimal":null,
                "low":"low",
                "medium":null,
                "high":"high",
                "xhigh":null,
                "max":"max"
            })
        );
    }

    #[test]
    fn does_not_infer_thinking_off_from_effort_list() {
        let got = run_helper(r#"[{"type":"effort","values":["low","high","max"]}]"#);
        assert_eq!(
            got,
            serde_json::json!({
                "off":null,
                "minimal":null,
                "low":"low",
                "medium":null,
                "high":"high",
                "xhigh":null,
                "max":"max"
            })
        );
    }

    #[test]
    fn leaves_toggle_budget_and_unverified_controls_to_adapters() {
        assert_eq!(
            run_helper(r#"[{"type":"toggle"}]"#),
            serde_json::Value::Null
        );
        assert_eq!(
            run_helper(r#"[{"type":"budget_tokens","min":1024,"max":32000}]"#),
            serde_json::Value::Null
        );
        assert_eq!(
            run_helper(r#"[{"type":"effort","values":[null,"default"]}]"#),
            serde_json::Value::Null
        );
    }
}
