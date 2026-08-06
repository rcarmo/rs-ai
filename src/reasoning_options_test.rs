//! Test-for-test adaptation of upstream `test/reasoning-options.test.ts`.
//!
//! The upstream helper lives in the model-generation script. rs-ai stores the
//! result as `Model.thinking_level_map`; these tests pin the same mapping policy.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[derive(Clone)]
    enum Control<'a> {
        Toggle,
        BudgetTokens,
        Effort(Vec<Option<&'a str>>),
    }

    fn get_effort_thinking_level_map(
        controls: &[Control<'_>],
    ) -> Option<HashMap<&'static str, Option<String>>> {
        let values = controls.iter().find_map(|control| match control {
            Control::Effort(values) => Some(values),
            _ => None,
        })?;
        let verified = ["none", "low", "medium", "high", "xhigh", "max"];
        if values
            .iter()
            .flatten()
            .any(|value| !verified.contains(value))
        {
            return None;
        }
        let has = |needle: &str| values.iter().flatten().any(|value| *value == needle);
        Some(HashMap::from([
            (
                "off",
                if controls.iter().any(|c| matches!(c, Control::Toggle)) && has("none") {
                    Some("none".to_string())
                } else {
                    None
                },
            ),
            ("minimal", None),
            ("low", has("low").then(|| "low".to_string())),
            ("medium", has("medium").then(|| "medium".to_string())),
            ("high", has("high").then(|| "high".to_string())),
            ("xhigh", has("xhigh").then(|| "xhigh".to_string())),
            ("max", has("max").then(|| "max".to_string())),
        ]))
    }

    #[test]
    fn exposes_only_verified_effort_values_and_none() {
        let got = get_effort_thinking_level_map(&[
            Control::Toggle,
            Control::Effort(vec![Some("none"), Some("low"), Some("high"), Some("max")]),
        ])
        .unwrap();
        assert_eq!(got["off"].as_deref(), Some("none"));
        assert_eq!(got["minimal"], None);
        assert_eq!(got["low"].as_deref(), Some("low"));
        assert_eq!(got["medium"], None);
        assert_eq!(got["high"].as_deref(), Some("high"));
        assert_eq!(got["xhigh"], None);
        assert_eq!(got["max"].as_deref(), Some("max"));
    }

    #[test]
    fn does_not_infer_thinking_off_from_effort_list() {
        let got = get_effort_thinking_level_map(&[Control::Effort(vec![
            Some("low"),
            Some("high"),
            Some("max"),
        ])])
        .unwrap();
        assert_eq!(got["off"], None);
        assert_eq!(got["low"].as_deref(), Some("low"));
        assert_eq!(got["high"].as_deref(), Some("high"));
        assert_eq!(got["max"].as_deref(), Some("max"));
    }

    #[test]
    fn leaves_toggle_budget_and_unverified_controls_to_adapters() {
        assert!(get_effort_thinking_level_map(&[Control::Toggle]).is_none());
        assert!(get_effort_thinking_level_map(&[Control::BudgetTokens]).is_none());
        assert!(
            get_effort_thinking_level_map(&[Control::Effort(vec![None, Some("default")])])
                .is_none()
        );
    }
}
