#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod base;
pub use base::BASE_MAINNET;
use op_revm::OpSpecId;

/// Base stack hardforks in chronological order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hardfork {
    /// Bedrock - initial Base stack launch
    Bedrock,
    /// Canyon hardfork
    Canyon,
    /// Delta hardfork
    Delta,
    /// Ecotone hardfork
    Ecotone,
    /// Fjord hardfork
    Fjord,
    /// Granite hardfork
    Granite,
    /// Holocene hardfork
    Holocene,
    /// Isthmus hardfork
    Isthmus,
    /// Jovian hardfork
    Jovian,
}

impl Hardfork {
    /// Convert to the corresponding `OpSpecId`
    #[must_use]
    pub const fn to_spec_id(self) -> OpSpecId {
        match self {
            Self::Bedrock => OpSpecId::BEDROCK,
            Self::Canyon | Self::Delta => OpSpecId::CANYON,
            Self::Ecotone => OpSpecId::ECOTONE,
            Self::Fjord => OpSpecId::FJORD,
            Self::Granite => OpSpecId::GRANITE,
            Self::Holocene => OpSpecId::HOLOCENE,
            Self::Isthmus => OpSpecId::ISTHMUS,
            Self::Jovian => OpSpecId::JOVIAN,
        }
    }
}

/// Hardfork activation timestamps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hardforks {
    /// Canyon hardfork activation timestamp
    pub canyon: u64,
    /// Delta hardfork activation timestamp
    pub delta: u64,
    /// Ecotone hardfork activation timestamp
    pub ecotone: u64,
    /// Fjord hardfork activation timestamp
    pub fjord: u64,
    /// Granite hardfork activation timestamp
    pub granite: u64,
    /// Holocene hardfork activation timestamp
    pub holocene: u64,
    /// Isthmus hardfork activation timestamp
    pub isthmus: u64,
    /// Jovian hardfork activation timestamp
    pub jovian: u64,
}

impl Hardforks {
    /// Get the activation timestamp for a hardfork
    #[must_use]
    pub const fn activation_time(&self, hardfork: Hardfork) -> u64 {
        match hardfork {
            Hardfork::Bedrock => 0,
            Hardfork::Canyon => self.canyon,
            Hardfork::Delta => self.delta,
            Hardfork::Ecotone => self.ecotone,
            Hardfork::Fjord => self.fjord,
            Hardfork::Granite => self.granite,
            Hardfork::Holocene => self.holocene,
            Hardfork::Isthmus => self.isthmus,
            Hardfork::Jovian => self.jovian,
        }
    }

    /// Check if a hardfork is active at the given timestamp
    #[must_use]
    pub const fn is_active(&self, hardfork: Hardfork, timestamp: u64) -> bool {
        timestamp >= self.activation_time(hardfork)
    }
}

/// Chain specification for a Base stack chain
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chain {
    /// Chain ID
    pub chain_id: u64,
    /// Chain name
    pub name: &'static str,
    /// Hardfork activation timestamps
    pub hardforks: Hardforks,
}

impl Chain {
    /// Get the chain ID
    #[must_use]
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get the chain name
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Get the hardforks configuration
    #[must_use]
    pub const fn hardforks(&self) -> &Hardforks {
        &self.hardforks
    }

    /// Check if a hardfork is active at the given timestamp
    #[must_use]
    pub const fn is_active(&self, hardfork: Hardfork, timestamp: u64) -> bool {
        self.hardforks.is_active(hardfork, timestamp)
    }

    /// Get the activation timestamp for a hardfork
    #[must_use]
    pub const fn activation_time(&self, hardfork: Hardfork) -> u64 {
        self.hardforks.activation_time(hardfork)
    }

    /// Get the highest active hardfork at the given timestamp
    #[must_use]
    pub const fn active_hardfork(&self, timestamp: u64) -> Hardfork {
        if self.is_active(Hardfork::Jovian, timestamp) {
            Hardfork::Jovian
        } else if self.is_active(Hardfork::Isthmus, timestamp) {
            Hardfork::Isthmus
        } else if self.is_active(Hardfork::Holocene, timestamp) {
            Hardfork::Holocene
        } else if self.is_active(Hardfork::Granite, timestamp) {
            Hardfork::Granite
        } else if self.is_active(Hardfork::Fjord, timestamp) {
            Hardfork::Fjord
        } else if self.is_active(Hardfork::Ecotone, timestamp) {
            Hardfork::Ecotone
        } else if self.is_active(Hardfork::Delta, timestamp) {
            Hardfork::Delta
        } else if self.is_active(Hardfork::Canyon, timestamp) {
            Hardfork::Canyon
        } else {
            Hardfork::Bedrock
        }
    }

    /// Derive the correct `OpSpecId` for a given block timestamp
    #[must_use]
    pub const fn spec_id_at_timestamp(&self, timestamp: u64) -> OpSpecId {
        self.active_hardfork(timestamp).to_spec_id()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(1_704_992_400, OpSpecId::BEDROCK, "before Canyon")]
    #[case(BASE_MAINNET.hardforks.canyon, OpSpecId::CANYON, "at Canyon")]
    #[case(BASE_MAINNET.hardforks.ecotone, OpSpecId::ECOTONE, "at Ecotone")]
    #[case(BASE_MAINNET.hardforks.fjord, OpSpecId::FJORD, "at Fjord")]
    #[case(BASE_MAINNET.hardforks.granite, OpSpecId::GRANITE, "at Granite")]
    #[case(BASE_MAINNET.hardforks.holocene, OpSpecId::HOLOCENE, "at Holocene")]
    #[case(BASE_MAINNET.hardforks.isthmus, OpSpecId::ISTHMUS, "at Isthmus")]
    #[case(BASE_MAINNET.hardforks.jovian, OpSpecId::JOVIAN, "at Jovian")]
    fn base_mainnet_spec_id(
        #[case] timestamp: u64,
        #[case] expected: OpSpecId,
        #[case] _description: &str,
    ) {
        assert_eq!(BASE_MAINNET.spec_id_at_timestamp(timestamp), expected);
    }

    #[test]
    fn chain_properties() {
        assert_eq!(BASE_MAINNET.chain_id(), 8453);
        assert_eq!(BASE_MAINNET.name(), "Base Mainnet");
    }

    #[rstest]
    #[case(Hardfork::Jovian, Hardfork::Isthmus)]
    #[case(Hardfork::Isthmus, Hardfork::Holocene)]
    #[case(Hardfork::Holocene, Hardfork::Granite)]
    #[case(Hardfork::Granite, Hardfork::Fjord)]
    #[case(Hardfork::Fjord, Hardfork::Ecotone)]
    #[case(Hardfork::Ecotone, Hardfork::Delta)]
    #[case(Hardfork::Delta, Hardfork::Canyon)]
    #[case(Hardfork::Canyon, Hardfork::Bedrock)]
    fn hardfork_ordering(#[case] later: Hardfork, #[case] earlier: Hardfork) {
        assert!(later > earlier);
    }

    #[test]
    fn bedrock_always_active() {
        assert!(BASE_MAINNET.is_active(Hardfork::Bedrock, 0));
    }

    #[rstest]
    #[case(BASE_MAINNET.hardforks.canyon - 1, false, "before activation")]
    #[case(BASE_MAINNET.hardforks.canyon, true, "at activation")]
    #[case(BASE_MAINNET.hardforks.canyon + 1, true, "after activation")]
    fn canyon_activation(
        #[case] timestamp: u64,
        #[case] expected: bool,
        #[case] _description: &str,
    ) {
        assert_eq!(BASE_MAINNET.is_active(Hardfork::Canyon, timestamp), expected);
    }

    #[rstest]
    #[case(0, Hardfork::Bedrock)]
    #[case(BASE_MAINNET.hardforks.canyon, Hardfork::Canyon)]
    #[case(BASE_MAINNET.hardforks.ecotone, Hardfork::Ecotone)]
    #[case(BASE_MAINNET.hardforks.jovian, Hardfork::Jovian)]
    fn active_hardfork(#[case] timestamp: u64, #[case] expected: Hardfork) {
        assert_eq!(BASE_MAINNET.active_hardfork(timestamp), expected);
    }
}
