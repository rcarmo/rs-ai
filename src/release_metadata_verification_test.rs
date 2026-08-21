//! Executable full-metadata release verification gate.
//!
//! This complements the provider/id pair comparator by regenerating the complete
//! release-pinned Rust text/image registries and comparing all Rust-representable
//! metadata after generated-timestamp normalization.

#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn release_metadata_verifier_clean_run_succeeds_with_expected_counts() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/verify_release_model_metadata.py")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "clean verifier failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("text=1267"), "unexpected stdout: {stdout}");
        assert!(
            stdout.contains("providers=39"),
            "unexpected stdout: {stdout}"
        );
        assert!(stdout.contains("apis=9"), "unexpected stdout: {stdout}");
        assert!(
            stdout.contains("batchAliases=60"),
            "unexpected stdout: {stdout}"
        );
        assert!(stdout.contains("image=45"), "unexpected stdout: {stdout}");
    }

    fn run_fault(fault: &str) -> String {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/verify_release_model_metadata.py")
            .arg("--fault")
            .arg(fault)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "fault injection unexpectedly passed"
        );
        String::from_utf8_lossy(&output.stderr).to_string()
    }

    #[test]
    fn release_metadata_verifier_detects_fault_injected_text_metadata() {
        let stderr = run_fault("text-name");
        assert!(
            stderr.contains("text metadata mismatch"),
            "unexpected verifier stderr: {stderr}"
        );
        assert!(
            stderr.contains("Qwen3.8 Max FAULT"),
            "faulted value should be reported: {stderr}"
        );
    }

    #[test]
    fn release_metadata_verifier_detects_fault_injected_image_metadata() {
        let stderr = run_fault("image-name");
        assert!(
            stderr.contains("image metadata mismatch"),
            "unexpected verifier stderr: {stderr}"
        );
        assert!(
            stderr.contains("FLUX.2 Flex FAULT"),
            "faulted value should be reported: {stderr}"
        );
    }

    #[test]
    fn v0842_manifest_validator_confirms_changed_paths_and_crosswalk_rows() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/validate_v0842_manifests.py")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "manifest validator failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("changedPaths=42 testRows=131"),
            "unexpected stdout: {stdout}"
        );
    }
}
