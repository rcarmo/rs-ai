//! Current v1.1.0 catalog counts and retained v1.0.1 behavioral regressions.

#[cfg(test)]
mod tests {
    use crate::{classifier_models_generated, images, models_generated};

    #[test]
    fn native_version_and_exact_catalog_counts_match_v110() {
        assert_eq!(env!("CARGO_PKG_VERSION"), "1.1.0");
        assert_eq!(models_generated::builtin_models().len(), 1563);
        assert_eq!(images::models_generated::builtin_image_models().len(), 61);
        assert_eq!(
            classifier_models_generated::builtin_classifier_models().len(),
            26
        );
    }

    #[test]
    fn v101_catalog_additions_and_stale_ids_are_exact() {
        let chat = models_generated::builtin_models();
        assert!(chat.iter().any(|model| {
            model.provider == "together" && model.id == "deepseek-ai/DeepSeek-V4-Pro-0813"
        }));
        assert!(!chat.iter().any(|model| {
            model.provider == "together" && model.id == "deepseek-ai/DeepSeek-V4-Pro"
        }));
        assert!(chat.iter().any(|model| {
            model.provider == "cloudflare-ai-gateway" && model.id.contains("claude-sonnet-4-5")
        }));

        let images = images::models_generated::builtin_image_models();
        for id in [
            "black-forest-labs/flux-3-image",
            "bytedance-seed/seedream-5-0-flash",
        ] {
            assert!(
                images.iter().any(|model| model.id == id),
                "missing image {id}"
            );
        }

        let classifiers = classifier_models_generated::builtin_classifier_models();
        for id in ["@cf/cloudflare/clef", "@cf/cloudflare/clef-flash"] {
            assert!(
                classifiers.iter().any(|model| model.id == id),
                "missing classifier {id}"
            );
        }
    }
}
