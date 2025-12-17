//! Re-export of database traits from revm

pub use revm::database_interface::{
    DBErrorMarker, Database as RevmDatabase, DatabaseCommit, DatabaseRef,
};

use crate::errors::DbError;

/// A combined database trait that includes all database functionality
/// plus block commitment capabilities.
pub trait Database:
    RevmDatabase<Error = DbError> + DatabaseCommit + DatabaseRef<Error = DbError> + Clone
{
    /// Commits the current block state.
    fn commit_block(&mut self);
}
