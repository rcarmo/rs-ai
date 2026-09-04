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
        assert!(stdout.contains("text=1336"), "unexpected stdout: {stdout}");
        assert!(
            stdout.contains("providers=39"),
            "unexpected stdout: {stdout}"
        );
        assert!(stdout.contains("apis=9"), "unexpected stdout: {stdout}");
        assert!(
            stdout.contains("batchAliases=66"),
            "unexpected stdout: {stdout}"
        );
        assert!(stdout.contains("image=50"), "unexpected stdout: {stdout}");
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

    #[test]
    fn v0843_manifest_validator_confirms_changed_paths_and_crosswalk_rows() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/validate_v0843_manifests.py")
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
            stdout.contains("changedPaths=48 testRows=136"),
            "unexpected stdout: {stdout}"
        );
    }

    #[test]
    fn v0844_manifest_validator_confirms_changed_paths_and_crosswalk_rows() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/validate_v0844_manifests.py")
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
            stdout.contains("changedPaths=15 testRows=137"),
            "unexpected stdout: {stdout}"
        );
    }

    #[test]
    fn v0850_baseline_delta_validator_confirms_full_record_counts() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/verify_v0850_baseline_delta.py")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "baseline delta validator failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("text=+72/-26/79 changed image=+0/-0/0 changed"),
            "unexpected stdout: {stdout}"
        );
    }

    #[test]
    fn v0850_baseline_delta_validator_detects_record_mutation() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/verify_v0850_baseline_delta.py")
            .arg("--fault")
            .arg("baseline-record")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(!output.status.success(), "fault unexpectedly passed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("text full-record delta mismatch"),
            "unexpected stderr: {stderr}"
        );
    }

    #[test]
    fn v0850_manifest_validator_confirms_changed_paths_and_crosswalk_rows() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/validate_v0850_manifests.py")
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
            stdout.contains("changedPaths=51 testRows=142"),
            "unexpected stdout: {stdout}"
        );
        assert!(
            stdout.contains(
                "changedSha256=db461a56838926cf60d4ae0196ed98fcc215616dacff013ad8c235bb8ad9b83f"
            ),
            "unexpected stdout: {stdout}"
        );
        assert!(
            stdout.contains(
                "testCorpusSha256=56f8742065a4ad01d73e5aee53035324f2e7333a735222ab15db870819e29065"
            ),
            "unexpected stdout: {stdout}"
        );
    }

    #[test]
    fn v0850_manifest_validator_detects_changed_path_inventory_corruption() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/validate_v0850_manifests.py")
            .arg("--fault")
            .arg("v0850-changed-paths")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(!output.status.success(), "fault unexpectedly passed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("v0850-changed-paths.txt sha256 mismatch"),
            "unexpected stderr: {stderr}"
        );
    }

    #[test]
    fn v0850_manifest_validator_detects_test_corpus_inventory_corruption() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/validate_v0850_manifests.py")
            .arg("--fault")
            .arg("v0850-test-corpus-142")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert!(!output.status.success(), "fault unexpectedly passed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("v0850-test-corpus-142.txt sha256 mismatch"),
            "unexpected stderr: {stderr}"
        );
    }
}
