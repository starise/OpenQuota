mod catalog;
mod codecs;
mod model_pricing;
mod rates;
mod store;
mod supplement;

pub use catalog::PricingCatalog;
pub use model_pricing::ModelPricing;
pub use rates::{ModelRates, TokenBreakdown};
pub use store::PricingStore;
pub use supplement::PricingSupplement;

#[cfg(test)]
pub fn test_bundled_pricing() -> ModelPricing {
    let rates = |input, cache_write, cache_read, output| {
        let mut rates = ModelRates::new(input, output);
        rates.cache_write_per_million = cache_write;
        rates.cache_read_per_million = cache_read;
        rates
    };
    let entries = |values: &[(&str, ModelRates)]| {
        values
            .iter()
            .map(|(model, rates)| ((*model).to_owned(), *rates))
            .collect()
    };
    ModelPricing::new(
        PricingSupplement::decode(include_bytes!("../../resources/pricing_supplement.json"))
            .expect("bundled pricing supplement must be valid"),
        PricingCatalog {
            entries: entries(&[
                ("claude-sonnet-4-5-20250929", rates(3.0, 3.75, 0.3, 15.0)),
                ("claude-sonnet-4-6", rates(3.0, 3.75, 0.3, 15.0)),
                ("claude-opus-4-6", rates(5.0, 6.25, 0.5, 25.0)),
                ("claude-fable-5", rates(10.0, 12.5, 1.0, 50.0)),
                ("gpt-5.4", rates(2.5, 2.5, 0.25, 15.0)),
                ("gpt-5.3-codex", rates(1.75, 1.75, 0.175, 14.0)),
            ]),
            retrieved_at: None,
        },
        PricingCatalog {
            entries: entries(&[("grok-build-0.1", rates(1.0, 1.0, 0.2, 2.0))]),
            retrieved_at: None,
        },
    )
}

#[cfg(test)]
mod bundled_resource_tests {
    use super::{test_bundled_pricing, TokenBreakdown};

    #[test]
    fn bundled_supplement_is_valid_and_fixture_catalogs_are_fixed() {
        let pricing = test_bundled_pricing();
        assert_eq!(pricing.primary.entries.len(), 6);
        assert_eq!(pricing.secondary.entries.len(), 1);
        assert!(!pricing.supplement.pricing.is_empty());
        assert!(!pricing.supplement.alias_rules.is_empty());
        let raw_supplement: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../resources/pricing_supplement.json"))
                .unwrap();
        assert_eq!(
            pricing.supplement.alias_rules.len(),
            raw_supplement["alias_rules"].as_array().unwrap().len(),
            "every bundled alias regex must compile in Rust"
        );
        assert_eq!(
            pricing
                .resolve("claude-opus-4-6")
                .unwrap()
                .input_per_million,
            5.0
        );
        assert_eq!(
            pricing.resolve("grok-build").unwrap().input_per_million,
            1.0
        );
    }

    #[test]
    fn known_log_models_and_aliases_match_expected_rates() {
        let pricing = test_bundled_pricing();
        let assert_rates = |model: &str, expected: [f64; 4]| {
            let rates = pricing.resolve(model).unwrap_or_else(|| panic!("{model}"));
            assert_eq!(
                [
                    rates.input_per_million,
                    rates.cache_write_per_million,
                    rates.cache_read_per_million,
                    rates.output_per_million,
                ],
                expected,
                "{model}"
            );
        };
        assert_eq!(
            pricing
                .resolve("claude-sonnet-4-5-20250929")
                .unwrap()
                .input_per_million,
            3.0
        );
        assert_eq!(
            pricing
                .resolve("gpt-5.6-sol-ultra-fast")
                .unwrap()
                .input_per_million,
            10.0
        );
        assert_eq!(
            pricing
                .resolve("claude-fable-5-thinking-xhigh")
                .unwrap()
                .output_per_million,
            50.0
        );
        assert_rates("gpt-5.6-high-fast", [10.0, 12.5, 1.0, 60.0]);
        assert_rates("gpt-daybreak-blue-latest", [5.0, 6.25, 0.5, 30.0]);
        assert_rates("daybreak-blue-latest", [5.0, 6.25, 0.5, 30.0]);
        assert_rates("gpt-5.6-terra-max-fast", [4.0, 5.0, 0.4, 24.0]);
        assert_rates("gpt-5.6-luna-high-fast", [0.4, 0.5, 0.04, 2.4]);
        assert_rates(
            "claude-4.6-opus-high-thinking-fast",
            [30.0, 37.5, 3.0, 150.0],
        );
        assert_rates("claude-opus-5", [5.0, 6.25, 0.5, 25.0]);
        assert_rates(
            "claude-opus-5[1m]-thinking-max-fast",
            [10.0, 12.5, 1.0, 50.0],
        );
        assert_rates("claude-sonnet-5", [2.0, 2.5, 0.2, 10.0]);
        assert_rates("kimi-k3-code-max", [3.0, 3.0, 0.3, 15.0]);
        assert_rates("cursor-grok-4.5-high-fast", [4.0, 4.0, 1.0, 12.0]);
        assert_rates("cursor-grok-4.6-medium", [2.0, 2.0, 0.5, 6.0]);
        assert_rates("grok-4.6-fast-high", [4.0, 4.0, 1.0, 12.0]);
        assert_eq!(
            pricing.resolve("grok-build-0.1").unwrap().input_per_million,
            1.0
        );
        assert_eq!(
            pricing
                .resolve("cursor-grok-4.5-xhigh")
                .unwrap()
                .input_per_million,
            pricing.resolve("grok-4.5").unwrap().input_per_million
        );
        assert_eq!(
            pricing
                .resolve("cursor-grok-4.5-fast-xhigh")
                .unwrap()
                .output_per_million,
            pricing.resolve("grok-4.5-fast").unwrap().output_per_million
        );
        assert!(pricing.resolve("glm-5.2-bogus").is_none());
    }

    #[test]
    fn fixture_engine_prices_cache_and_long_context_buckets() {
        let pricing = test_bundled_pricing();
        let cost = pricing
            .estimated_cost_dollars(
                "claude-sonnet-4-5-20250929",
                TokenBreakdown {
                    input: 100_000,
                    cache_write_5m: 60_000,
                    cache_read: 50_000,
                    output: 20_000,
                    ..TokenBreakdown::default()
                },
                true,
            )
            .unwrap();
        assert!(cost > 0.0);
    }
}
