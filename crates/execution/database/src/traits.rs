//! Re-export of database traits from revm

pub use revm::database_interface::{
    DBErrorMarker, Database as RevmDatabase, DatabaseCommit, DatabaseRef,
};
use revm::state::EvmState;
use alloy::primitives::B256;

use crate::errors::DbError;

/// A combined database trait that includes all database functionality
/// plus block commitment capabilities.
pub trait Database:
    RevmDatabase<Error = DbError> + DatabaseCommit + DatabaseRef<Error = DbError> + Clone
{
    /// Commits the block state changes and returns the new state root.
    fn commit_block(&mut self, transaction_changes: Vec<EvmState>) -> Result<B256, DbError>;
}
