//! Block executor implementation

use alloy::{consensus::BlockHeader, network::BlockResponse, primitives::B256};
use blocksource::{OpBlock, block_to_env, tx_to_op_tx};
use chainspec::Chain;
use database::Database;
use derive_more::Display;
use op_alloy::consensus::OpTxEnvelope;
use op_revm::{DefaultOp, L1BlockInfo, OpBuilder, OpSpecId::OSAKA};
use op_revm::OpSpecId::JOVIAN;
use revm::{
    ExecuteEvm,
    context::CfgEnv,
    context_interface::result::{ExecutionResult, Output},
    state::EvmState,
};
use tracing::{error, info};

/// Trait for block execution, allowing type-erased usage.
///
/// This trait abstracts over the database type, allowing validators and other
/// components to use block execution without being generic over the database.
pub trait Executor: Send + Sync {
    /// Execute all transactions in a block and commit state changes.
    fn execute_block(&mut self, block: OpBlock) -> Result<BlockResult, ExecutorError>;
}

/// Errors that can occur during block execution
#[derive(Debug, Display)]
pub enum ExecutorError {
    /// EVM execution error
    #[display("EVM execution error: {_0}")]
    Evm(String),
    /// Database error
    #[display("Database error: {_0}")]
    Database(String),
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
    /// State changes from this transaction
    pub state: EvmState,
}

/// Result of executing a block
#[derive(Debug, Clone)]
pub struct BlockResult {
    /// Block number
    pub block_number: u64,
    /// Individual transaction results
    pub tx_results: Vec<TxResult>,
    /// State root after executing this block
    pub state_root: B256,
}

/// Block executor that uses op-revm to execute Base stack blocks
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

        // let spec_id = self.chain.spec_id_at_timestamp(timestamp);
        let block_env = block_to_env(&block);

        let mut cfg =
            CfgEnv::new().with_chain_id(self.chain.chain_id()).with_spec(JOVIAN);

        // let mut cfg = CfgEnv::default();
        // cfg.spec = OSAKA;
        // cfg.spec = spec_id;
        // cfg.chain_id = self.chain.chain_id();
        cfg.disable_eip3607 = true;

        let transactions = block.transactions.into_transactions();
        let mut tx_results = Vec::with_capacity(tx_count);

        for (idx, tx) in transactions.enumerate() {
            let tx_result =
                self.execute_tx(&tx, idx, tx_count, &block_env, &cfg).inspect_err(|e| {
                    error!(block = block_number, tx_idx = idx, tx_hash = %tx.inner.inner.tx_hash(), "Transaction execution failed: {e}")
                })?;
            tx_results.push(tx_result);
        }

        // Collect all transaction state changes
        let transaction_changes: Vec<EvmState> =
            tx_results.iter().map(|r| r.state.clone()).collect();

        // Commit all transaction changes at once and get the state root
        let state_root = self
            .db
            .commit_block(block_number, transaction_changes)
            .map_err(|e| ExecutorError::Database(e.to_string()))?;

        Ok(BlockResult { block_number, tx_results, state_root })
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
                // Call commit to update in-memory caches (code cache, etc.)
                self.db.commit(result.state.clone());

                let tx_result = match result.result {
                    ExecutionResult::Success { ref output, gas_used, .. } => {
                        let output_bytes = match output {
                            Output::Call(data) | Output::Create(data, _) => data.to_vec(),
                        };
                        TxResult {
                            success: true,
                            gas_used,
                            output: Some(output_bytes),
                            state: result.state,
                        }
                    }
                    ExecutionResult::Revert { ref output, gas_used } => TxResult {
                        success: false,
                        gas_used,
                        output: Some(output.to_vec()),
                        state: result.state,
                    },
                    ExecutionResult::Halt { reason: _, gas_used } => {
                        TxResult { success: false, gas_used, output: None, state: result.state }
                    }
                };
                Ok(tx_result)
            }
            Err(e) => Err(ExecutorError::Evm(e.to_string())),
        }
    }
}

impl<DB> Executor for BlockExecutor<DB>
where
    DB: Database + Send + Sync,
{
    fn execute_block(&mut self, block: OpBlock) -> Result<BlockResult, ExecutorError> {
        Self::execute_block(self, block)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use alloy::{
        consensus::{Header, Signed, TxEip1559, transaction::Recovered},
        genesis::{Genesis, GenesisAccount},
        primitives::{Address, Bytes, Signature, TxKind, U256, address, b256},
        rpc::types::{Block, BlockTransactions, Header as RpcHeader},
    };
    use alloy_trie::EMPTY_ROOT_HASH;
    use chainspec::BASE_MAINNET;
    use database::{CachedDatabase, RocksDbKvDatabase, TrieDatabase};
    use op_alloy::{consensus::OpTxEnvelope, rpc_types::Transaction as OpTransaction};
    use revm::DatabaseRef;

    use super::*;

    /// Create a genesis with prefunded accounts
    fn test_genesis() -> Genesis {
        let alice = address!("0x1111111111111111111111111111111111111111");
        let bob = address!("0x2222222222222222222222222222222222222222");

        let mut alloc = BTreeMap::new();
        alloc.insert(
            alice,
            GenesisAccount {
                balance: U256::from(1_000_000_000_000_000_000_000u128), // 1000 ETH
                nonce: Some(0),
                code: None,
                storage: None,
                private_key: None,
            },
        );
        alloc.insert(
            bob,
            GenesisAccount {
                balance: U256::ZERO,
                nonce: Some(0),
                code: None,
                storage: None,
                private_key: None,
            },
        );

        Genesis { alloc, ..Default::default() }
    }

    /// Create a mock signed EIP-1559 transaction for ETH transfer
    fn create_eth_transfer_tx(
        from: Address,
        to: Address,
        value: U256,
        nonce: u64,
        chain_id: u64,
    ) -> OpTransaction {
        // Create EIP-1559 transaction
        let tx = TxEip1559 {
            chain_id,
            nonce,
            gas_limit: 21000,               // Standard ETH transfer gas
            max_fee_per_gas: 1_000_000_000, // 1 gwei
            max_priority_fee_per_gas: 1_000_000_000,
            to: TxKind::Call(to),
            value,
            input: Bytes::default(),
            access_list: Default::default(),
        };

        // Create a mock signature (this is a dummy signature for testing)
        // In a real scenario, you'd sign with a private key
        let signature = Signature::new(U256::from(1), U256::from(2), false);

        let tx_hash = b256!("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef");
        let signed = Signed::new_unchecked(tx, signature, tx_hash);

        // Wrap in OpTxEnvelope
        let envelope = OpTxEnvelope::Eip1559(signed);

        // Create the recovered transaction (envelope + sender)
        let recovered = Recovered::new_unchecked(envelope, from);

        // Create the RPC transaction wrapper
        let alloy_tx = alloy::rpc::types::Transaction {
            inner: recovered,
            block_hash: Some(B256::ZERO),
            block_number: Some(1),
            transaction_index: Some(0),
            effective_gas_price: Some(1_000_000_000),
        };

        OpTransaction { inner: alloy_tx, deposit_nonce: None, deposit_receipt_version: None }
    }

    /// Create a mock block header
    fn create_block_header(block_number: u64, timestamp: u64, base_fee: u64) -> RpcHeader {
        let inner = Header {
            number: block_number,
            timestamp,
            gas_limit: 30_000_000,
            base_fee_per_gas: Some(base_fee),
            beneficiary: Address::ZERO,
            excess_blob_gas: Some(0),
            blob_gas_used: Some(0),
            ..Default::default()
        };
        let hash = inner.hash_slow();
        RpcHeader { hash, inner, total_difficulty: None, size: None }
    }

    /// Create a block with a single transaction
    fn create_block_with_tx(tx: OpTransaction, block_number: u64, timestamp: u64) -> OpBlock {
        let header = create_block_header(block_number, timestamp, 1_000_000_000);

        Block {
            header,
            uncles: vec![],
            transactions: BlockTransactions::Full(vec![tx]),
            withdrawals: None,
        }
    }

    #[test]
    fn test_execute_block_with_eth_transfer() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("test_db");
        let kvdb_path = temp_dir.path().join("kvdb");

        // Create genesis with prefunded accounts
        let genesis = test_genesis();
        let alice = address!("0x1111111111111111111111111111111111111111");
        let bob = address!("0x2222222222222222222222222222222222222222");

        // Create RocksDB key-value database
        let kvdb =
            RocksDbKvDatabase::open_or_create(&kvdb_path, &genesis).expect("failed to create kvdb");

        // Create TrieDatabase with genesis
        let trie_db = TrieDatabase::open_or_create(&db_path, &genesis, kvdb.clone())
            .expect("failed to create database");

        // Wrap in CachedDatabase
        let cached_db = CachedDatabase::new(trie_db);

        // Verify initial state
        let alice_initial = cached_db.basic_ref(alice).unwrap().expect("alice should exist");
        assert_eq!(alice_initial.balance, U256::from(1_000_000_000_000_000_000_000u128));
        assert_eq!(alice_initial.nonce, 0);

        let bob_initial = cached_db.basic_ref(bob).unwrap().expect("bob should exist");
        assert_eq!(bob_initial.balance, U256::ZERO);

        // Create ETH transfer transaction: Alice sends 1 ETH to Bob
        let transfer_value = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
        let tx = create_eth_transfer_tx(alice, bob, transfer_value, 0, BASE_MAINNET.chain_id());

        // Create block with the transaction
        // Use a timestamp that activates Isthmus (after all hardforks)
        let block_timestamp = BASE_MAINNET.hardforks.isthmus + 1;
        let block = create_block_with_tx(tx, 1, block_timestamp);

        // Execute the block
        let mut executor = BlockExecutor::new(cached_db.clone(), BASE_MAINNET);
        let result = executor.execute_block(block).expect("block execution failed");

        // Verify block result
        assert_eq!(result.block_number, 1);
        assert_eq!(result.tx_results.len(), 1);

        // Verify transaction succeeded
        let tx_result = &result.tx_results[0];
        assert!(tx_result.success, "ETH transfer should succeed");
        assert_eq!(tx_result.gas_used, 21000, "Standard ETH transfer uses 21000 gas");

        // Verify state root is not empty
        assert_ne!(result.state_root, EMPTY_ROOT_HASH, "state root should not be empty");
        assert_ne!(result.state_root, B256::ZERO, "state root should not be zero");

        // Drop the executor to release the database
        drop(executor);
        drop(cached_db);

        // Re-open the database to verify state was persisted
        let trie_db = TrieDatabase::open_or_create(&db_path, &genesis, kvdb)
            .expect("failed to re-open database");

        // Alice should have:
        // - Balance reduced by transfer_value + gas_cost
        // - Nonce incremented to 1
        let alice_final = trie_db.basic_ref(alice).unwrap().expect("alice should exist");
        assert_eq!(alice_final.nonce, 1, "Alice's nonce should be incremented");

        let gas_cost = U256::from(21000u64 * 1_000_000_000u64); // gas_used * gas_price
        let expected_alice_balance =
            U256::from(1_000_000_000_000_000_000_000u128) - transfer_value - gas_cost;
        assert_eq!(
            alice_final.balance, expected_alice_balance,
            "Alice's balance should be reduced by transfer + gas"
        );

        // Bob should have received the transfer
        let bob_final = trie_db.basic_ref(bob).unwrap().expect("bob should exist");
        assert_eq!(bob_final.balance, transfer_value, "Bob should have received the ETH");
    }
}
