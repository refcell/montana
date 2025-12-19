use crate::{Chain, Hardforks};

/// Chain specification for local Anvil development network.
pub const ANVIL: Chain = Chain {
    chain_id: 31337,
    name: "Anvil",
    hardforks: Hardforks {
        // Canyon activation: January 11, 2024 at 17:00:01 UTC
        canyon: 1_704_992_401,
        // Delta activation: February 22, 2024 at 00:00:00 UTC
        delta: 1_708_560_000,
        // Ecotone activation: March 14, 2024 at 00:00:01 UTC
        ecotone: 1_710_374_401,
        // Fjord activation: July 10, 2024 at 16:00:01 UTC
        fjord: 1_720_627_201,
        // Granite activation: September 11, 2024 at 16:00:01 UTC
        granite: 1_726_070_401,
        // Holocene activation: January 9, 2025 at 18:00:01 UTC
        holocene: 1_736_445_601,
        // Isthmus activation: May 9, 2025 at 16:00:01 UTC
        isthmus: 1_746_806_401,
        // Jovian activation: December 2, 2025 at 16:00:01 UTC
        jovian: 1_764_777_601,
    },
};
