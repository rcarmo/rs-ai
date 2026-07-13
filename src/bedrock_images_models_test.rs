//! Test-for-test ports of the deterministic cases of upstream
//! `test/bedrock-models.test.ts` and `test/images-models.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2). The live per-model request cases and the
//! instance ImagesModels-collection cases are out of scope here.

#[cfg(test)]
mod tests {
    use crate::images::models_generated::builtin_image_models;
    use crate::registry::list_models;

    #[test]
    fn gets_all_available_bedrock_models() {
        assert!(
            !list_models(Some("amazon-bedrock")).is_empty(),
            "the Bedrock catalog must be non-empty"
        );
    }

    #[test]
    fn builtin_image_models_register_the_openrouter_provider_with_its_catalog() {
        let models = builtin_image_models();
        assert!(!models.is_empty(), "the image catalog must be non-empty");
        assert!(
            models.iter().all(|m| m.api == "openrouter-images"),
            "all image models use the openrouter-images api"
        );
        assert!(
            models.iter().all(|m| m.provider == "openrouter"),
            "the only built-in image provider is openrouter"
        );
    }
}
