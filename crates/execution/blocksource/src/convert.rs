//! Conversion functions for blocks and transactions.

use alloy::{
    consensus::{BlockHeader, Transaction as ConsensusTx, Typed2718},
    eips::eip2718::Encodable2718,
    network::BlockResponse,
    primitives::{Address, Bytes, TxKind, U256},
};
use op_alloy::{consensus::OpTxEnvelope, rpc_types::Transaction as OpRpcTransaction};
use op_revm::transaction::{OpTransaction, deposit::DepositTransactionParts};
use revm::{
    context::{
        BlockEnv, TxEnv,
        either::Either,
        transaction::{AccessList, AccessListItem},
    },
    context_interface::block::BlobExcessGasAndPrice,
    primitives::eip4844::BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN,
};

use crate::types::OpBlock;

/// Convert an alloy block to revm `BlockEnv`
#[must_use]
pub fn block_to_env(block: &OpBlock) -> BlockEnv {
    BlockEnv {
        number: U256::from(block.number()),
        beneficiary: block.header.beneficiary(),
        timestamp: U256::from(block.header().timestamp()),
        gas_limit: block.header().gas_limit(),
        basefee: block.header().base_fee_per_gas().unwrap_or(0),
        difficulty: block.header().difficulty(),
        prevrandao: block.header().mix_hash(),
        blob_excess_gas_and_price: block
            .header()
            .excess_blob_gas()
            .map(|excess| BlobExcessGasAndPrice::new(excess, BLOB_BASE_FEE_UPDATE_FRACTION_CANCUN)),
    }
}

/// Convert an op-alloy transaction to `OpTransaction<TxEnv>`
#[must_use]
pub fn tx_to_op_tx(tx: &OpRpcTransaction, sender: Address) -> OpTransaction<TxEnv> {
    // Get the consensus envelope to access deposit fields
    let envelope: &OpTxEnvelope = tx.inner.inner.inner();

    // Extract tx_type
    let tx_type = envelope.ty();

    // Extract destination
    let kind = envelope.to().map_or(TxKind::Create, TxKind::Call);

    // Build access list - convert alloy AccessListItem to revm AccessList
    let access_list: AccessList = envelope
        .access_list()
        .map(|al| {
            let items: Vec<AccessListItem> = al
                .iter()
                .map(|item| AccessListItem {
                    address: item.address,
                    storage_keys: item.storage_keys.clone(),
                })
                .collect();
            AccessList::from(items)
        })
        .unwrap_or_default();

    let authorization_list = tx
        .authorization_list()
        .unwrap_or_default()
        .iter()
        .map(|auth| Either::Left(auth.clone()))
        .collect();

    let base = TxEnv {
        tx_type,
        caller: sender,
        gas_limit: envelope.gas_limit(),
        gas_price: envelope.gas_price().unwrap_or_else(|| envelope.max_fee_per_gas()),
        kind,
        value: envelope.value(),
        data: envelope.input().clone(),
        nonce: envelope.nonce(),
        chain_id: envelope.chain_id(),
        access_list,
        gas_priority_fee: envelope.max_priority_fee_per_gas(),
        blob_hashes: envelope.blob_versioned_hashes().unwrap_or_default().to_vec(),
        max_fee_per_blob_gas: envelope.max_fee_per_blob_gas().unwrap_or(0),
        authorization_list,
    };

    // Check if this is a deposit transaction and extract real fields
    let (deposit, enveloped_tx) = envelope.as_deposit().map_or_else(
        || {
            // Non-deposit transactions require the RLP-encoded envelope for L1 cost computation
            let encoded = envelope.encoded_2718();
            (DepositTransactionParts::default(), Some(Bytes::from(encoded)))
        },
        |deposit_tx| {
            // Extract real deposit fields from the op-alloy TxDeposit
            let inner = deposit_tx.inner();
            (
                DepositTransactionParts {
                    source_hash: inner.source_hash,
                    mint: Some(inner.mint),
                    is_system_transaction: inner.is_system_transaction,
                },
                None, // Deposit transactions don't need enveloped bytes
            )
        },
    );

    OpTransaction { base, enveloped_tx, deposit }
}
