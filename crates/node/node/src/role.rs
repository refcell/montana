/// The operational role of the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeRole {
    /// Executes blocks and submits batches to L1
    Sequencer,
    /// Derives and validates blocks from L1
    Validator,
    /// Runs both sequencer and validator concurrently
    #[default]
    Dual,
}

impl NodeRole {
    /// Returns true if this role runs the sequencer
    pub const fn runs_sequencer(&self) -> bool {
        matches!(self, Self::Sequencer | Self::Dual)
    }

    /// Returns true if this role runs the validator
    pub const fn runs_validator(&self) -> bool {
        matches!(self, Self::Validator | Self::Dual)
    }
}
