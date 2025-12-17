//! Block executor implementation

use alloy::{consensus::BlockHeader, network::BlockResponse};
use blocksource::{OpBlock, block_to_env, tx_to_op_tx};
use chainspec::Chain;
use database::Database;
use derive_more::Display;
use op_alloy::consensus::OpTxEnvelope;
use op_revm::{DefaultOp, L1BlockInfo, OpBuilder};
use revm::{
    ExecuteEvm,
    context::CfgEnv,
    context_interface::result::{ExecutionResult, Output},
};
use tracing::info;

/// Errors that can occur during block execution
#[derive(Debug, Display)]
pub enum ExecutorError {
    /// EVM execution error
    #[display("EVM execution error: {_0}")]
    Evm(String),
}

impl std::error::Error for ExecutorError {}

/// Result of executing a single transaction
#[derive(Debug, Clone)]
pub struct TxResult {
    /// Whether the transaction succeeded
    pub success: bool,
    /// Gas used by the transaction
    pub gas_used: u64,
    /// Output data (for successful calls/creates)
    pub output: Option<Vec<u8>>,
}

/// Result of executing a block
#[derive(Debug, Clone)]
pub struct BlockResult {
    /// Block number
    pub block_number: u64,
    /// Individual transaction results
    pub tx_results: Vec<TxResult>,
}

/// Block executor that uses op-revm to execute OP Stack blocks
#[derive(Debug)]
pub struct BlockExecutor<DB> {
    db: DB,
    chain: Chain,
}

impl<DB> BlockExecutor<DB>
where
    DB: Database,
{
    /// Create a new block executor
    pub const fn new(db: DB, chain: Chain) -> Self {
        Self { db, chain }
    }

    /// Execute all transactions in a block
    pub fn execute_block(&mut self, block: OpBlock) -> Result<BlockResult, ExecutorError> {
        let block_number = block.header().number();
        let tx_count = block.transactions.len();
        let timestamp = block.header().timestamp();

        info!(
            "Executing block {} with {} transactions, timestamp: {}",
            block_number, tx_count, timestamp
        );

        let spec_id = self.chain.spec_id_at_timestamp(timestamp);
        let block_env = block_to_env(&block);

        let mut cfg = CfgEnv::default();
        cfg.spec = spec_id;
        cfg.chain_id = self.chain.chain_id();

        let transactions = block.transactions.into_transactions();
        let mut tx_results = Vec::with_capacity(tx_count);

        for (idx, tx) in transactions.enumerate() {
            let result = self.execute_tx(&tx, idx, tx_count, &block_env, &cfg)?;
            tx_results.push(result);
        }

        Ok(BlockResult { block_number, tx_results })
    }

    fn execute_tx(
        &mut self,
        tx: &op_alloy::rpc_types::Transaction,
        _idx: usize,
        _tx_count: usize,
        block_env: &revm::context::BlockEnv,
        cfg: &CfgEnv<op_revm::OpSpecId>,
    ) -> Result<TxResult, ExecutorError> {
        let envelope: &OpTxEnvelope = tx.inner.inner.inner();
        let _tx_hash = envelope.tx_hash();
        let sender = tx.inner.inner.signer();
        let op_tx = tx_to_op_tx(tx, sender);

        let ctx = revm::Context::op()
            .with_db(self.db.clone())
            .with_block(block_env.clone())
            .with_tx(op_tx)
            .with_cfg(cfg.clone())
            .with_chain(L1BlockInfo::default());

        let mut evm = ctx.build_op();

        match evm.replay() {
            Ok(result) => {
                self.db.commit(result.state);

                let tx_result = match result.result {
                    ExecutionResult::Success { ref output, gas_used, .. } => {
                        let output_bytes = match output {
                            Output::Call(data) | Output::Create(data, _) => data.to_vec(),
                        };
                        TxResult { success: true, gas_used, output: Some(output_bytes) }
                    }
                    ExecutionResult::Revert { ref output, gas_used } => {
                        TxResult { success: false, gas_used, output: Some(output.to_vec()) }
                    }
                    ExecutionResult::Halt { reason: _, gas_used } => {
                        TxResult { success: false, gas_used, output: None }
                    }
                };
                Ok(tx_result)
            }
            Err(e) => Err(ExecutorError::Evm(e.to_string())),
        }
    }
}
