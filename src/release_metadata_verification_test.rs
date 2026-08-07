//! Executable full-metadata release verification gate.
//!
//! This complements the provider/id pair comparator by regenerating the complete
//! release-pinned Rust text/image registries and comparing all Rust-representable
//! metadata after generated-timestamp normalization.

#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn release_metadata_verifier_detects_fault_injected_text_metadata() {
        let output = Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg("-B")
            .arg("scripts/verify_release_model_metadata.py")
            .arg("--fault")
            .arg("text-name")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();

        assert!(
            !output.status.success(),
            "fault injection unexpectedly passed"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("text metadata mismatch"),
            "unexpected verifier stderr: {stderr}"
        );
        assert!(
            stderr.contains("Qwen3.8 Max FAULT"),
            "faulted value should be reported: {stderr}"
        );
    }
}
