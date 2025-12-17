//! Derivation runner configuration.

/// Derivation runner configuration.
#[derive(Clone, Debug)]
pub struct DerivationConfig {
    /// Poll interval for checking new batches in milliseconds (default: 100ms).
    pub poll_interval_ms: u64,
}

impl Default for DerivationConfig {
    fn default() -> Self {
        Self { poll_interval_ms: 100 }
    }
}

impl DerivationConfig {
    /// Creates a new builder for configuring a derivation runner.
    pub fn builder() -> DerivationConfigBuilder {
        DerivationConfigBuilder::default()
    }
}

/// Builder for [`DerivationConfig`].
#[derive(Clone, Debug)]
pub struct DerivationConfigBuilder {
    poll_interval_ms: u64,
}

impl Default for DerivationConfigBuilder {
    fn default() -> Self {
        let defaults = DerivationConfig::default();
        Self { poll_interval_ms: defaults.poll_interval_ms }
    }
}

impl DerivationConfigBuilder {
    /// Sets the poll interval in milliseconds.
    pub const fn poll_interval_ms(mut self, poll_interval_ms: u64) -> Self {
        self.poll_interval_ms = poll_interval_ms;
        self
    }

    /// Builds the [`DerivationConfig`].
    pub const fn build(self) -> DerivationConfig {
        DerivationConfig { poll_interval_ms: self.poll_interval_ms }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn derivation_config_default() {
        let config = DerivationConfig::default();
        assert_eq!(config.poll_interval_ms, 100);
    }

    #[rstest]
    #[case(50, "50ms poll interval")]
    #[case(100, "default 100ms poll interval")]
    #[case(200, "200ms poll interval")]
    fn derivation_config_poll_interval(#[case] interval: u64, #[case] _description: &str) {
        let config = DerivationConfig { poll_interval_ms: interval };
        assert_eq!(config.poll_interval_ms, interval);
    }

    #[test]
    fn derivation_config_clone() {
        let config = DerivationConfig { poll_interval_ms: 150 };
        let cloned = config.clone();
        assert_eq!(cloned.poll_interval_ms, config.poll_interval_ms);
    }

    #[test]
    fn derivation_config_debug() {
        let config = DerivationConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("DerivationConfig"));
        assert!(debug_str.contains("poll_interval_ms"));
    }

    // Builder tests

    #[test]
    fn builder_default() {
        let config = DerivationConfig::builder().build();
        let default_config = DerivationConfig::default();
        assert_eq!(config.poll_interval_ms, default_config.poll_interval_ms);
    }

    #[test]
    fn builder_poll_interval_ms() {
        let config = DerivationConfig::builder().poll_interval_ms(200).build();
        assert_eq!(config.poll_interval_ms, 200);
    }

    #[test]
    fn builder_clone() {
        let builder = DerivationConfig::builder().poll_interval_ms(150);
        let cloned = builder.clone();
        let config1 = builder.build();
        let config2 = cloned.build();
        assert_eq!(config1.poll_interval_ms, config2.poll_interval_ms);
    }

    #[test]
    fn builder_debug() {
        let builder = DerivationConfig::builder();
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("DerivationConfigBuilder"));
    }
}
