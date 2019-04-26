//! A middleware for JSONRPC and Muta blockchain.
#![allow(clippy::needless_lifetimes)]

use futures::compat::Future01CompatExt;
use std::sync::Arc;

use core_context::Context;
use core_merkle::{self, Merkle, ProofNode};
use core_runtime::{ExecutionContext, Executor, TransactionPool};
use core_serialization::AsyncCodec;
use core_storage::Storage;
use core_types::{Address, Block, BloomRef, Hash, Receipt, SignedTransaction};
use log;

use crate::cita::{self, Uint};
use crate::error::RpcError;
use crate::filter::Filter;
use crate::util;
use crate::RpcResult;
use numext_fixed_hash::H256;
use numext_fixed_uint::U256;

pub struct AppState<E, T, S> {
    executor:         Arc<E>,
    transaction_pool: Arc<T>,
    storage:          Arc<S>,
}

impl<E, T, S> Clone for AppState<E, T, S> {
    fn clone(&self) -> Self {
        Self {
            executor:         Arc::<E>::clone(&self.executor),
            transaction_pool: Arc::<T>::clone(&self.transaction_pool),
            storage:          Arc::<S>::clone(&self.storage),
        }
    }
}

impl<E, T, S> AppState<E, T, S>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
{
    pub fn new(executor: Arc<E>, transaction_pool: Arc<T>, storage: Arc<S>) -> Self {
        Self {
            executor,
            transaction_pool,
            storage,
        }
    }
}

/// Help functions for rpc APIs.
impl<E, T, S> AppState<E, T, S>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
{
    pub async fn get_block(&self, number: String) -> RpcResult<Block> {
        let h = await!(self.get_height(number))?;
        let b = await!(self.storage.get_block_by_height(Context::new(), h).compat())?;
        Ok(b)
    }

    pub async fn get_block_by_tx_hash(&self, tx_hash: Hash) -> RpcResult<Block> {
        let p = await!(self
            .storage
            .get_transaction_position(Context::new(), &tx_hash)
            .compat())?;
        let b = await!(self
            .storage
            .get_block_by_hash(Context::new(), &p.block_hash)
            .compat())?;
        Ok(b)
    }

    pub async fn get_height(&self, number: String) -> RpcResult<u64> {
        match &number.to_ascii_lowercase()[..] {
            "earliest" => Ok(0),
            "latest" | "pending" | "" => {
                let b = await!(self.storage.get_latest_block(Context::new()).compat())?;
                Ok(b.header.height)
            }
            x => {
                let h = util::clean_0x(x);
                Ok(u64::from_str_radix(h, 16).map_err(|e| RpcError::Str(format!("{:?}", e)))?)
            }
        }
    }

    /// Convert muta::Block => cita::Block
    pub async fn ret_cita_block(
        &self,
        raw_block: Block,
        include_tx: bool,
    ) -> RpcResult<cita::Block> {
        let mut res_block = cita::Block {
            version: 0,
            hash:    raw_block.header.hash(),
            header:  cita::BlockHeader {
                timestamp:         raw_block.header.timestamp,
                prev_hash:         raw_block.header.prevhash,
                number:            Uint::from(raw_block.header.height),
                state_root:        raw_block.header.state_root,
                transactions_root: raw_block.header.transactions_root,
                receipts_root:     raw_block.header.receipts_root,
                quota_used:        Uint::from(raw_block.header.quota_used),
                proof:             None,
                proposer:          raw_block.header.proposer,
            },
            body:    cita::BlockBody {
                transactions: raw_block
                    .tx_hashes
                    .iter()
                    .map(|hash| cita::BlockTransaction::Hash(hash.clone()))
                    .collect(),
            },
        };
        if !include_tx {
            return Ok(res_block);
        }

        let raw_txs = await!(self
            .storage
            .get_transactions(Context::new(), &raw_block.tx_hashes)
            .compat())?;
        let mut txs = vec![];
        for tx in raw_txs {
            txs.push(cita::BlockTransaction::Full(cita::FullTransaction {
                hash:    tx.hash,
                content: tx.untx.transaction.data,
                from:    tx.sender,
            }));
        }
        res_block.body.transactions = txs;
        Ok(res_block)
    }

    /// Convert muta::Receipt => cita::Receipt
    pub async fn ret_cita_receipt(&self, raw_receipt: Receipt) -> RpcResult<cita::Receipt> {
        let b = await!(self.get_block_by_tx_hash(raw_receipt.transaction_hash.clone()))?;
        let receipts = await!(self
            .storage
            .get_receipts(Context::new(), &b.tx_hashes[..])
            .compat())?;
        let mut logs_in_block_before_receipt = 0;
        let mut tx_index = 0;
        let tx_hash = raw_receipt.transaction_hash.clone();
        for r in receipts {
            if r.transaction_hash == tx_hash {
                break;
            }
            logs_in_block_before_receipt += r.logs.len();
            tx_index += 1;
        }
        let logs: Vec<cita::Log> = raw_receipt
            .logs
            .iter()
            .enumerate()
            .map(|(log_index, log_entry)| cita::Log {
                address:               log_entry.address.clone(),
                topics:                log_entry.topics.clone(),
                data:                  cita::Data::from(log_entry.data.clone()),
                block_hash:            Some(b.header.hash()),
                block_number:          Some(b.header.height.into()),
                transaction_hash:      Some(raw_receipt.transaction_hash.clone()),
                transaction_index:     Some(tx_index.into()),
                log_index:             Some(Uint::from(log_index as u64)),
                transaction_log_index: Some(Uint::from(
                    (log_index + logs_in_block_before_receipt) as u64,
                )),
            })
            .collect();
        let receipt = cita::Receipt {
            transaction_hash: Some(raw_receipt.transaction_hash.clone()),
            transaction_index: Some(tx_index.into()),
            block_hash: Some(b.hash.clone()),
            block_number: Some(b.header.height.into()),
            cumulative_quota_used: 0.into(), // TODO
            quota_used: Some(raw_receipt.quota_used.into()),
            contract_address: raw_receipt.contract_address.clone(),
            logs,
            state_root: Some(raw_receipt.state_root),
            logs_bloom: raw_receipt.logs_bloom,
            error_message: Some(raw_receipt.receipt_error.clone()),
        };
        Ok(receipt)
    }

    /// Convert muta::Transaction => cita::Transaction
    pub async fn ret_cita_transaction(
        &self,
        raw_tx: SignedTransaction,
    ) -> RpcResult<cita::RpcTransaction> {
        let mut tx = cita::RpcTransaction {
            hash:         raw_tx.hash.clone(),
            content:      raw_tx.untx.transaction.data.clone(),
            from:         raw_tx.sender.clone(),
            block_number: Uint::from(0),
            block_hash:   Hash::from_bytes(&[0x00u8; 32]).unwrap(),
            index:        Uint::from(0),
        };
        let receipt = await!(self
            .storage
            .get_receipt(Context::new(), &raw_tx.hash)
            .compat());
        let receipt = match receipt {
            Ok(ok) => ok,
            Err(_) => return Ok(tx),
        };
        let b = await!(self
            .storage
            .get_block_by_hash(Context::new(), &receipt.block_hash)
            .compat())?;
        tx.block_number = Uint::from(b.header.height);
        tx.block_hash = receipt.block_hash;
        tx.index = Uint::from(b.tx_hashes.iter().position(|x| x == &raw_tx.hash).unwrap() as u64);
        Ok(tx)
    }
}

/// Async rpc APIs.
/// See ./server.rs::rpc_select to learn about meanings of these APIs.
impl<E, T, S> AppState<E, T, S>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
{
    pub async fn block_number(&self) -> RpcResult<u64> {
        let b = await!(self.storage.get_latest_block(Context::new()).compat())?;
        Ok(b.header.height)
    }

    pub async fn call(
        &self,
        number: String,
        call_request: cita::CallRequest,
    ) -> RpcResult<cita::Data> {
        let b = await!(self.get_block(number))?;
        let rd_result = self.executor.readonly(
            Context::new(),
            &ExecutionContext {
                state_root:  b.header.state_root,
                proposer:    b.header.proposer,
                height:      b.header.height,
                quota_limit: b.header.quota_limit,
                timestamp:   b.header.timestamp,
            },
            &Address::from_bytes(Into::<Vec<u8>>::into(call_request.to).as_slice())
                .expect("never returns an error"),
            &Address::from_bytes(
                call_request
                    .from
                    .map_or(vec![0x00u8; 20], Into::into)
                    .as_slice(),
            )
            .expect("never returns an error"),
            &call_request.data.map_or(vec![], Into::into),
        )?;
        Ok(cita::Data::from(rd_result.data.unwrap_or_default()))
    }

    pub async fn get_abi(&self, _addr: Address, _block_number: String) -> RpcResult<Vec<u8>> {
        // TODO. Can't implement at now
        unimplemented!()
    }

    pub async fn get_balance(&self, number: String, addr: Address) -> RpcResult<U256> {
        let b = await!(self.get_block(number))?;
        let balance = self
            .executor
            .get_balance(Context::new(), &b.header.state_root, &addr)?;
        Ok(balance)
    }

    pub async fn get_block_by_hash(&self, hash: Hash, include_tx: bool) -> RpcResult<cita::Block> {
        let b = await!(self
            .storage
            .get_block_by_hash(Context::new(), &hash)
            .compat())?;
        let r = await!(self.ret_cita_block(b, include_tx))?;
        Ok(r)
    }

    pub async fn get_block_by_number(
        &self,
        number: String,
        include_tx: bool,
    ) -> RpcResult<cita::Block> {
        let b = await!(self.get_block(number))?;
        let r = await!(self.ret_cita_block(b, include_tx))?;
        Ok(r)
    }

    pub async fn get_block_header(&self, number: String) -> RpcResult<Vec<u8>> {
        let b = await!(self.get_block(number))?;
        Ok(rlp::encode(&b.header))
    }

    pub async fn get_code(&self, address: Address, number: String) -> RpcResult<Vec<u8>> {
        let b = await!(self.get_block(number))?;
        let (code, _code_hash) =
            self.executor
                .get_code(Context::new(), &b.header.state_root, &address)?;
        Ok(code)
    }

    pub async fn get_logs(&self, filter: cita::Filter) -> RpcResult<Vec<cita::Log>> {
        let filter: Filter = filter.into();
        let possible_blooms = filter.bloom_possibilities();
        let from_block = await!(self.get_height(filter.from_block.clone()))?;
        let to_block = await!(self.get_height(filter.to_block.clone()))?;

        let mut blocks: Vec<Block> = Vec::new();
        for block_height in from_block..=to_block {
            let block = await!(self
                .storage
                .get_block_by_height(Context::new(), block_height)
                .compat())?;
            blocks.push(block)
        }
        let filtered_blocks = blocks
            .into_iter()
            .filter(|block| {
                possible_blooms
                    .iter()
                    .any(|bloom| bloom.contains_bloom(BloomRef::from(&block.header.logs_bloom)))
            })
            .collect::<Vec<_>>();

        let mut tx_hashes: Vec<Hash> = vec![];
        for block in &filtered_blocks[..] {
            tx_hashes.extend(block.tx_hashes.clone());
        }
        if tx_hashes.is_empty() {
            return Ok(vec![]);
        }
        let receipts_res = await!(self
            .storage
            .get_receipts(Context::new(), tx_hashes.as_slice())
            .compat())?;

        let mut logs = vec![];
        let mut tx_idx = 0;
        for block in &filtered_blocks[..] {
            let mut log_index: usize = 0;
            for tx_hash in &block.tx_hashes {
                let mut _transaction_index = 0;
                let receipt = &receipts_res[tx_idx];
                let receipt_contains_bloom = possible_blooms
                    .iter()
                    .any(|bloom| bloom.contains_bloom(BloomRef::from(&receipt.logs_bloom)));
                if receipt_contains_bloom {
                    for log_entry in &receipt.logs {
                        let mut _transaction_log_index = 0;
                        if filter.matches(&log_entry) {
                            let log = cita::Log {
                                address:               log_entry.address.clone(),
                                topics:                log_entry.topics.clone(),
                                data:                  cita::Data::from(log_entry.data.clone()),
                                block_hash:            Some(block.header.hash()),
                                block_number:          Some(block.header.height.into()),
                                transaction_hash:      Some(tx_hash.clone()),
                                transaction_index:     Some(_transaction_index.into()),
                                log_index:             Some(Uint::from(log_index as u64)),
                                transaction_log_index: Some(_transaction_log_index.into()),
                            };
                            logs.push(log);
                        }
                        _transaction_log_index += 1;
                        log_index += 1;
                    }
                } else {
                    log_index += &receipt.logs.len();
                }
                _transaction_index += 1;
                tx_idx += 1;
            }
        }

        let len = logs.len();
        let logs = match filter.limit {
            Some(limit) if len >= limit => logs.split_off(len - limit),
            _ => logs,
        };
        Ok(logs)
    }

    pub async fn get_metadata(&self, _number: String) -> RpcResult<cita::MetaData> {
        // TODO. Can't implement at now
        Ok(cita::MetaData::default())
    }

    pub async fn get_state_proof(
        &self,
        _addr: Address,
        _key: Hash,
        _block_number: String,
    ) -> RpcResult<Vec<u8>> {
        // TODO. Can't implement at now
        unimplemented!()
    }

    pub async fn get_storage_at(
        &self,
        addr: Address,
        key: H256,
        number: String,
    ) -> RpcResult<Vec<u8>> {
        let b = await!(self.get_block(number))?;
        let r = self
            .executor
            .get_value(Context::new(), &b.header.state_root, &addr, &key)?;
        Ok(r.as_bytes().to_vec())
    }

    pub async fn get_transaction(&self, hash: Hash) -> RpcResult<cita::RpcTransaction> {
        let tx = await!(self.storage.get_transaction(Context::new(), &hash).compat())?;
        let tx_cita = await!(self.ret_cita_transaction(tx))?;
        Ok(tx_cita)
    }

    pub async fn get_transaction_count(&self, addr: Address, number: String) -> RpcResult<U256> {
        let b = await!(self.get_block(number))?;
        let r = self
            .executor
            .get_nonce(Context::new(), &b.header.state_root, &addr)?;
        Ok(r)
    }

    pub async fn get_transaction_proof(&self, _hash: Hash) -> RpcResult<Vec<u8>> {
        // TODO. Can't implement at now
        unimplemented!()
    }

    pub async fn get_receipt_proof(&self, tx_hash: Hash) -> RpcResult<Vec<ProofNode>> {
        let block = await!(self.get_block_by_tx_hash(tx_hash.clone()))?;
        let tx_hashes = block.tx_hashes;
        let index = tx_hashes
            .iter()
            .position(|x| x == &tx_hash)
            .expect("tx should be in block");
        let receipt_list = await!(self
            .storage
            .get_receipts(Context::new(), &tx_hashes[..])
            .compat())?;
        // get merkle proof
        let hahses: Vec<Hash> = receipt_list.iter().map(Receipt::hash).collect();
        let tree = Merkle::from_hashes(hahses.clone());
        Ok(tree
            .get_proof_by_input_index(index)
            .expect("should always return proof if index is correct"))
    }

    pub async fn get_transaction_receipt(&self, hash: Hash) -> RpcResult<cita::Receipt> {
        let r = await!(self.storage.get_receipt(Context::new(), &hash).compat())?;
        let cita_r = await!(self.ret_cita_receipt(r))?;
        Ok(cita_r)
    }

    pub async fn peer_count(&self) -> RpcResult<u32> {
        // TODO. Can't implement at now
        Ok(42)
    }

    pub async fn send_raw_transaction(&self, signed_data: Vec<u8>) -> RpcResult<cita::TxResponse> {
        let ser_untx = await!(AsyncCodec::decode::<cita::UnverifiedTransaction>(
            signed_data
        ))?;
        if ser_untx.transaction.is_none() {
            return Err(RpcError::Str("Transaction not found!".into()));
        };
        let ser_raw_tx = await!(AsyncCodec::encode(ser_untx.clone().transaction.unwrap()))?;
        let message = Hash::from_fixed_bytes(tiny_keccak::keccak256(&ser_raw_tx));
        let untx: core_types::transaction::UnverifiedTransaction = ser_untx.into();
        log::debug!("Accept {:?}", untx);
        let r = await!(self
            .transaction_pool
            .insert(Context::new(), message, untx)
            .compat());
        let r = match r {
            Ok(ok) => ok,
            Err(e) => {
                log::warn!("Insert to pool failed. {:?}", e);
                return Err(e.into());
            }
        };
        Ok(cita::TxResponse::new(r.hash, String::from("OK")))
    }
}
