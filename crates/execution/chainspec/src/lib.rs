//! Chain specification for OP Stack chains
//!
//! This module provides chain specifications including hardfork activation timestamps
//! and helpers for deriving the correct `OpSpecId` from a block timestamp.

mod base;

pub use base::BASE_MAINNET;
use op_revm::OpSpecId;

/// OP Stack hardforks in chronological order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hardfork {
    /// Bedrock - initial OP Stack launch
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

/// Chain specification for an OP Stack chain
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
    use super::*;

    #[test]
    fn test_base_mainnet_spec_id() {
        let chain = BASE_MAINNET;

        // Before Canyon = Bedrock
        assert_eq!(chain.spec_id_at_timestamp(1_704_992_400), OpSpecId::BEDROCK);

        // At Canyon
        assert_eq!(chain.spec_id_at_timestamp(chain.hardforks.canyon), OpSpecId::CANYON);

        // At Ecotone
        assert_eq!(chain.spec_id_at_timestamp(chain.hardforks.ecotone), OpSpecId::ECOTONE);

        // At Fjord
        assert_eq!(chain.spec_id_at_timestamp(chain.hardforks.fjord), OpSpecId::FJORD);

        // At Granite
        assert_eq!(chain.spec_id_at_timestamp(chain.hardforks.granite), OpSpecId::GRANITE);

        // At Holocene
        assert_eq!(chain.spec_id_at_timestamp(chain.hardforks.holocene), OpSpecId::HOLOCENE);

        // At Isthmus
        assert_eq!(chain.spec_id_at_timestamp(chain.hardforks.isthmus), OpSpecId::ISTHMUS);

        // At Jovian
        assert_eq!(chain.spec_id_at_timestamp(chain.hardforks.jovian), OpSpecId::JOVIAN);
    }

    #[test]
    fn test_chain_properties() {
        let chain = BASE_MAINNET;
        assert_eq!(chain.chain_id(), 8453);
        assert_eq!(chain.name(), "Base Mainnet");
    }

    #[test]
    fn test_hardfork_ordering() {
        assert!(Hardfork::Jovian > Hardfork::Isthmus);
        assert!(Hardfork::Isthmus > Hardfork::Holocene);
        assert!(Hardfork::Canyon > Hardfork::Bedrock);
    }

    #[test]
    fn test_is_active() {
        let chain = BASE_MAINNET;

        // Bedrock is always active
        assert!(chain.is_active(Hardfork::Bedrock, 0));

        // Canyon not active before activation
        assert!(!chain.is_active(Hardfork::Canyon, chain.hardforks.canyon - 1));

        // Canyon active at activation
        assert!(chain.is_active(Hardfork::Canyon, chain.hardforks.canyon));

        // Canyon active after activation
        assert!(chain.is_active(Hardfork::Canyon, chain.hardforks.canyon + 1));
    }

    #[test]
    fn test_active_hardfork() {
        let chain = BASE_MAINNET;

        assert_eq!(chain.active_hardfork(0), Hardfork::Bedrock);
        assert_eq!(chain.active_hardfork(chain.hardforks.canyon), Hardfork::Canyon);
        assert_eq!(chain.active_hardfork(chain.hardforks.ecotone), Hardfork::Ecotone);
        assert_eq!(chain.active_hardfork(chain.hardforks.jovian), Hardfork::Jovian);
    }
}
