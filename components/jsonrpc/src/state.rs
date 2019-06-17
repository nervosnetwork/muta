//! A middleware for JSONRPC and Muta blockchain.
#![allow(clippy::needless_lifetimes)]

use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use std::time::SystemTime;

use futures::compat::Future01CompatExt;
use futures_locks::RwLock;
use log;
use numext_fixed_hash::{H160, H256};
use numext_fixed_uint::U256;

use core_context::{Context, ORIGIN};
use core_crypto::{Crypto, CryptoTransform};
use core_merkle::{self, Merkle, ProofNode};
use core_runtime::network::PeerCount;
use core_runtime::{ExecutionContext, Executor, Storage, TransactionOrigin, TransactionPool};
use core_serialization::AsyncCodec;
use core_types::{Address, Block, BloomRef, Hash, Receipt, SignedTransaction};

use crate::cita::{self, Uint};
use crate::error::RpcError;
use crate::filter::Filter;
use crate::util;
use crate::RpcResult;

#[derive(Default)]
pub struct FilterDatabase {
    /// Self-increase ID.
    /// Note: Should use function `gen_id()` istead of touch it directly.
    next_available_id: u32,

    /// To save the filter for filter_logs
    regs: HashMap<u32, Filter>,
    /// To save the result for filter_logs
    data: HashMap<u32, Vec<cita::Log>>,
    /// To save the last update timestamp for filter_logs
    lastupdate: HashMap<u32, u64>,

    /// To save the filter for filter_blocks
    block_regs: HashMap<u32, u64>,
    /// To save the result for filter_blocks
    block_data: HashMap<u32, Vec<Hash>>,
    /// To save the last update timestamp for filter_blocks
    block_lastupdate: HashMap<u32, u64>,
}

impl FilterDatabase {
    /// Generate a new fresh id
    fn gen_id(&mut self) -> u32 {
        let id = self.next_available_id;
        self.next_available_id = self.next_available_id.wrapping_add(1);
        id
    }

    fn is_filter(&self, id: u32) -> bool {
        self.regs.contains_key(&id)
    }

    fn is_block_filter(&self, id: u32) -> bool {
        self.block_regs.contains_key(&id)
    }

    pub fn new_filter(&mut self, filter: Filter) -> u32 {
        let id = self.gen_id();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.regs.insert(id, filter);
        self.lastupdate.insert(id, now);
        id
    }

    pub fn new_block_filter(&mut self, start: u64) -> u32 {
        let id = self.gen_id();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.block_regs.insert(id, start);
        self.block_lastupdate.insert(id, now);
        id
    }

    // If there are any block filter, insert the block hash into
    // dataset.
    // recv_block in FilterDataBase is state independent.
    fn recv_block(&mut self, block: Block) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut uninstall_id = vec![];
        for (id, start_block_number) in self.block_regs.clone() {
            // Check if we should uninstall the filter
            let lastupdate = self.block_lastupdate.get(&id).expect("must exist");
            if now - lastupdate > 60 {
                uninstall_id.push(id);
                continue;
            }

            if block.header.height > start_block_number {
                let hashes = self.block_data.entry(id).or_insert_with(|| vec![]);
                hashes.push(block.hash.clone());
                continue;
            }
        }
        for id in uninstall_id {
            self.uninstall(id);
        }
    }

    pub fn uninstall(&mut self, id: u32) -> bool {
        if self.is_block_filter(id) {
            self.block_regs.remove(&id);
            self.block_data.remove(&id);
            self.block_lastupdate.remove(&id);
            true
        } else if self.is_filter(id) {
            self.regs.remove(&id);
            self.data.remove(&id);
            self.lastupdate.remove(&id);
            true
        } else {
            false
        }
    }

    pub fn filter_changes(&mut self, id: u32) -> Option<cita::FilterChanges> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if self.is_block_filter(id) {
            self.block_lastupdate.insert(id, now);
            let hashes = self.block_data.insert(id, vec![]).expect("must exist");
            Some(cita::FilterChanges::Hashes(hashes.clone()))
        } else if self.is_filter(id) {
            self.lastupdate.insert(id, now);
            let logs = self.data.insert(id, vec![]).expect("must exist");
            Some(cita::FilterChanges::Logs(logs))
        } else {
            None
        }
    }
}

pub struct AppState<E, T, S, C, P> {
    pub filterdb: Arc<RwLock<FilterDatabase>>,

    executor:         Arc<E>,
    transaction_pool: Arc<T>,
    storage:          Arc<S>,
    crypto:           Arc<C>,
    peer_count:       Arc<P>,
}

impl<E, T, S, C, P> Clone for AppState<E, T, S, C, P> {
    fn clone(&self) -> Self {
        Self {
            filterdb: Arc::<RwLock<FilterDatabase>>::clone(&self.filterdb),

            executor:         Arc::<E>::clone(&self.executor),
            transaction_pool: Arc::<T>::clone(&self.transaction_pool),
            storage:          Arc::<S>::clone(&self.storage),
            crypto:           Arc::<C>::clone(&self.crypto),
            peer_count:       Arc::<P>::clone(&self.peer_count),
        }
    }
}

impl<E, T, S, C, P> AppState<E, T, S, C, P>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
    P: PeerCount,
{
    pub fn new(
        executor: Arc<E>,
        transaction_pool: Arc<T>,
        storage: Arc<S>,
        crypto: Arc<C>,
        peer_count: Arc<P>,
    ) -> Self {
        Self {
            filterdb: Arc::new(RwLock::new(FilterDatabase::default())),

            executor,
            transaction_pool,
            storage,
            crypto,
            peer_count,
        }
    }
}

/// Help functions for rpc APIs.
impl<E, T, S, C, P> AppState<E, T, S, C, P>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
    P: PeerCount,
{
    pub async fn get_block(&self, number: String) -> RpcResult<Block> {
        let h = self.get_height(number).await?;
        let b = self.storage.get_block_by_height(Context::new(), h).await?;
        Ok(b)
    }

    pub async fn get_block_by_tx_hash(&self, tx_hash: Hash) -> RpcResult<Block> {
        let p = self
            .storage
            .get_transaction_position(Context::new(), &tx_hash)
            .await?;
        let b = self
            .storage
            .get_block_by_hash(Context::new(), &p.block_hash)
            .await?;
        Ok(b)
    }

    pub async fn get_height(&self, number: String) -> RpcResult<u64> {
        match &number.to_ascii_lowercase()[..] {
            "earliest" => Ok(0),
            "latest" | "pending" | "" => {
                let b = self.storage.get_latest_block(Context::new()).await?;
                Ok(b.header.height)
            }
            x => Ok(util::u64_from_string(x)?),
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
            hash:    raw_block.hash,
            header:  cita::BlockHeader {
                timestamp:         raw_block.header.timestamp * 1000, // ms
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

        let raw_txs = self
            .storage
            .get_transactions(Context::new(), &raw_block.tx_hashes)
            .await?;
        let mut txs = vec![];
        for tx in raw_txs {
            let cita_untx: cita::UnverifiedTransaction = From::from(tx.untx.clone());
            let content = AsyncCodec::encode(cita_untx).await?;
            txs.push(cita::BlockTransaction::Full(cita::FullTransaction {
                hash:    tx.hash.clone(),
                content: cita::Data::new(content),
                from:    tx.sender,
            }));
        }
        res_block.body.transactions = txs;
        Ok(res_block)
    }

    /// Convert muta::Receipt => cita::Receipt
    pub async fn ret_cita_receipt(&self, raw_receipt: Receipt) -> RpcResult<cita::Receipt> {
        let b = self
            .get_block_by_tx_hash(raw_receipt.transaction_hash.clone())
            .await?;
        let receipts = self
            .storage
            .get_receipts(Context::new(), &b.tx_hashes[..])
            .await?;
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
        let cita_untx: cita::UnverifiedTransaction = From::from(raw_tx.untx.clone());
        let content = AsyncCodec::encode(cita_untx).await?;
        let mut tx = cita::RpcTransaction {
            hash:         raw_tx.hash.clone(),
            content:      cita::Data::new(content),
            from:         raw_tx.sender.clone(),
            block_number: Uint::from(0),
            block_hash:   Hash::from_bytes(&[0x00u8; 32]).unwrap(),
            index:        Uint::from(0),
        };
        let b = self.get_block_by_tx_hash(raw_tx.hash.clone()).await?;
        tx.block_number = Uint::from(b.header.height);
        tx.block_hash = b.hash;
        tx.index = Uint::from(b.tx_hashes.iter().position(|x| x == &raw_tx.hash).unwrap() as u64);
        Ok(tx)
    }
}

/// Async rpc APIs.
/// See ./server.rs::rpc_select to learn about meanings of these APIs.
impl<E, T, S, C, P> AppState<E, T, S, C, P>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
    P: PeerCount,
{
    pub async fn block_number(&self) -> RpcResult<u64> {
        let b = self.storage.get_latest_block(Context::new()).await?;
        Ok(b.header.height)
    }

    pub async fn call(
        &self,
        number: String,
        call_request: cita::CallRequest,
    ) -> RpcResult<cita::Data> {
        let b = self.get_block(number).await?;
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

    pub async fn get_abi(&self, _addr: Address, _block_number: String) -> RpcResult<cita::Data> {
        // TODO. Can't implement at now
        Ok(cita::Data::new(vec![]))
    }

    pub async fn get_balance(&self, number: String, addr: Address) -> RpcResult<U256> {
        let b = self.get_block(number).await?;
        let balance = self
            .executor
            .get_balance(Context::new(), &b.header.state_root, &addr)?;
        Ok(balance)
    }

    pub async fn get_block_by_hash(&self, hash: Hash, include_tx: bool) -> RpcResult<cita::Block> {
        let b = self
            .storage
            .get_block_by_hash(Context::new(), &hash)
            .await?;
        let r = self.ret_cita_block(b, include_tx).await?;
        Ok(r)
    }

    pub async fn get_block_by_number(
        &self,
        number: String,
        include_tx: bool,
    ) -> RpcResult<cita::Block> {
        let b = self.get_block(number).await?;
        let r = self.ret_cita_block(b, include_tx).await?;
        Ok(r)
    }

    pub async fn get_block_header(&self, number: String) -> RpcResult<cita::Data> {
        let b = self.get_block(number).await?;
        Ok(cita::Data::new(rlp::encode(&b.header)))
    }

    pub async fn get_code(&self, address: Address, number: String) -> RpcResult<cita::Data> {
        let b = self.get_block(number).await?;
        let (code, _code_hash) = self
            .executor
            .get_code(Context::new(), &b.header.state_root, &address)
            .unwrap_or_default();
        Ok(cita::Data::new(code))
    }

    pub async fn get_logs(&self, filter: cita::Filter) -> RpcResult<Vec<cita::Log>> {
        let filter: Filter = filter.into();
        let possible_blooms = filter.bloom_possibilities();
        let from_block = self.get_height(filter.from_block.clone()).await?;
        let to_block = self.get_height(filter.to_block.clone()).await?;

        let mut logs = vec![];
        let mut log_index = 0;
        for block_height in from_block..=to_block {
            let block = self
                .storage
                .get_block_by_height(Context::new(), block_height)
                .await?;

            let mut fit = false;
            for bloom in &possible_blooms {
                if block
                    .header
                    .logs_bloom
                    .contains_bloom(BloomRef::from(bloom))
                {
                    fit = true;
                    break;
                }
            }

            if !fit {
                log_index += block.tx_hashes.len();
                continue;
            }
            let receipts_res = self
                .storage
                .get_receipts(Context::new(), block.tx_hashes.as_slice())
                .await?;

            for (tx_idx, tx_hash) in block.tx_hashes.iter().enumerate() {
                let receipt = &receipts_res[tx_idx];
                for (log_entry_index, log_entry) in receipt.logs.iter().enumerate() {
                    if filter.matches(&log_entry) {
                        let log = cita::Log {
                            address:               log_entry.address.clone(),
                            topics:                log_entry.topics.clone(),
                            data:                  cita::Data::from(log_entry.data.clone()),
                            block_hash:            Some(block.header.hash()),
                            block_number:          Some(block.header.height.into()),
                            transaction_hash:      Some(tx_hash.clone()),
                            transaction_index:     Some((tx_idx as u64).into()),
                            log_index:             Some(Uint::from(log_index as u64)),
                            transaction_log_index: Some((log_entry_index as u64).into()),
                        };
                        logs.push(log);
                    }

                    // Early return
                    if let Some(limit) = filter.limit {
                        if logs.len() >= limit {
                            return Ok(logs);
                        }
                    }

                    log_index += 1;
                }
            }
        }
        Ok(logs)
    }

    pub async fn get_metadata(&self, _number: String) -> RpcResult<cita::MetaData> {
        // TODO. Can't implement at now
        Ok(cita::MetaData {
            chain_id: 42,
            chain_id_v1: Uint::from(42),
            chain_name: String::from("Muta TestNet"),
            operator: String::from("Muta team"),
            website: String::from("https://github.com/cryptape"),
            genesis_timestamp: 0,
            validators: vec![
                cita::Data20::new(
                    H160::from_hex_str("19e49d3efd4e81dc82943ad9791c1916e2229138").unwrap(),
                ),
                cita::Data20::new(
                    H160::from_hex_str("2ae83ce578e4bb7968104b5d7c034af36a771a35").unwrap(),
                ),
                cita::Data20::new(
                    H160::from_hex_str("529dd2ef2dd117072b7e606b6a8ae111628f9108").unwrap(),
                ),
                cita::Data20::new(
                    H160::from_hex_str("7d14100eba2db1858e77a62d3d592b332a37a7a7").unwrap(),
                ),
            ],
            block_interval: 3000, // ms
            token_name: String::from("Mutcoin"),
            token_symbol: String::from("MUT"),
            token_avatar: String::from(
                "http://miamioh.edu/_files/images/ucm/resources/logo/print-M_186K.jpg",
            ),
            ..cita::MetaData::default()
        })
    }

    pub async fn get_state_proof(
        &self,
        addr: Address,
        key: Hash,
        number: String,
    ) -> RpcResult<Vec<u8>> {
        let b = self.get_block(number).await?;
        let state_root = &b.header.state_root;
        let account_proof = self
            .executor
            .get_account_proof(Context::new(), state_root, &addr)?;
        let storage_proof =
            self.executor
                .get_storage_proof(Context::new(), state_root, &addr, &key)?;
        if storage_proof.is_empty() {
            return Err(RpcError::StateProofNotFoundError);
        }
        let state_proof = cita::StateProof {
            address:       addr,
            account_proof: account_proof.into_iter().map(cita::Data::from).collect(),
            key:           key.clone(),
            value_proof:   storage_proof.into_iter().map(cita::Data::from).collect(),
        };

        Ok(rlp::encode(&state_proof))
    }

    pub async fn get_storage_at(
        &self,
        addr: Address,
        key: H256,
        number: String,
    ) -> RpcResult<cita::Data> {
        let b = self.get_block(number).await?;
        let r = self
            .executor
            .get_value(Context::new(), &b.header.state_root, &addr, &key)?;
        Ok(cita::Data::new(r.as_bytes().to_vec()))
    }

    pub async fn get_transaction(&self, hash: Hash) -> RpcResult<cita::RpcTransaction> {
        let tx = self.storage.get_transaction(Context::new(), &hash).await?;
        let tx_cita = self.ret_cita_transaction(tx).await?;
        Ok(tx_cita)
    }

    pub async fn get_transaction_count(&self, addr: Address, number: String) -> RpcResult<U256> {
        let b = self.get_block(number).await?;
        let txs = self
            .storage
            .get_transactions(Context::new(), b.tx_hashes.as_slice())
            .await?;
        let r = txs.iter().filter(|x| x.sender == addr).count();
        Ok(U256::from(r as u32))
    }

    pub async fn get_transaction_proof(&self, hash: Hash) -> RpcResult<cita::Data> {
        let block = self.get_block_by_tx_hash(hash.clone()).await?;
        let block_receipts = self
            .storage
            .get_receipts(Context::new(), block.tx_hashes.as_slice())
            .await?;
        let tx_index = block
            .tx_hashes
            .iter()
            .position(|x| x == &hash)
            .expect("block should always contains the transaction");
        let tx_receipt = &block_receipts[tx_index];
        assert_eq!(tx_receipt.transaction_hash, hash); // Must fit

        // Build the receipts merkle tree for block
        let tree = Merkle::from_hashes(
            block_receipts
                .iter()
                .map(|r| Hash::digest(rlp::encode(r).as_slice()))
                .collect(),
        );
        // Get proof
        let proof = tree
            .get_proof_by_input_index(tx_index)
            .expect("should always exists");

        // Done! Now we build the TxProof struct for CITA RPC response.
        // Get raw transaction
        let resp_tx = self.storage.get_transaction(Context::new(), &hash).await?;
        let resp_block_header = block.header;
        let resp_next_proposal_block = self
            .storage
            .get_block_by_height(Context::new(), resp_block_header.height + 1)
            .await?;
        let resp_next_proposal_header = resp_next_proposal_block.header;

        let resp_third_block = self
            .storage
            .get_block_by_height(Context::new(), resp_block_header.height + 1)
            .await?;
        let resp_third_proposal_proof = resp_third_block.header.proof.clone();

        let resp_proof: Vec<cita::ProofNode<Hash>> = proof
            .iter()
            .map(|x| cita::ProofNode {
                is_right: x.is_right,
                hash:     x.hash.clone(),
            })
            .collect();
        Ok(cita::Data::from(rlp::encode(&cita::TxProof {
            tx:                   resp_tx,
            receipt:              tx_receipt.clone(),
            receipt_proof:        resp_proof,
            block_header:         resp_block_header,
            next_proposal_header: resp_next_proposal_header,
            proposal_proof:       resp_third_proposal_proof,
        })))
    }

    pub async fn get_receipt_proof(&self, tx_hash: Hash) -> RpcResult<Vec<ProofNode>> {
        let block = self.get_block_by_tx_hash(tx_hash.clone()).await?;
        let tx_hashes = block.tx_hashes;
        let index = tx_hashes
            .iter()
            .position(|x| x == &tx_hash)
            .expect("tx should be in block");
        let receipt_list = self
            .storage
            .get_receipts(Context::new(), &tx_hashes[..])
            .await?;
        // get merkle proof
        let hahses: Vec<Hash> = receipt_list.iter().map(Receipt::hash).collect();
        let tree = Merkle::from_hashes(hahses.clone());
        Ok(tree
            .get_proof_by_input_index(index)
            .expect("should always return proof if index is correct"))
    }

    pub async fn get_transaction_receipt(&self, hash: Hash) -> RpcResult<cita::Receipt> {
        let r = self.storage.get_receipt(Context::new(), &hash).await?;
        let cita_r = self.ret_cita_receipt(r).await?;
        Ok(cita_r)
    }

    pub async fn peer_count(&self) -> RpcResult<u32> {
        Ok(self.peer_count.peer_count() as u32)
    }

    pub async fn send_raw_transaction(&self, signed_data: Vec<u8>) -> RpcResult<cita::TxResponse> {
        let ser_untx =
            AsyncCodec::decode::<cita::UnverifiedTransaction>(signed_data.clone()).await?;
        if ser_untx.transaction.is_none() {
            return Err(RpcError::Str("Transaction not found!".into()));
        };
        let untx: core_types::transaction::UnverifiedTransaction = ser_untx.try_into()?;
        let origin_ctx =
            Context::new().with_value::<TransactionOrigin>(ORIGIN, TransactionOrigin::Jsonrpc);
        log::debug!("Accept {:?}", untx);
        let r = self.transaction_pool.insert(origin_ctx, untx).await;
        let r = match r {
            Ok(ok) => ok,
            Err(e) => {
                log::warn!("Insert to pool failed. {:?}", e);
                return Err(e.into());
            }
        };
        Ok(cita::TxResponse::new(r.hash, String::from("OK")))
    }

    pub async fn send_unsafe_transaction(
        &self,
        tx_data: Vec<u8>,
        privkey: Vec<u8>,
    ) -> RpcResult<cita::TxResponse> {
        let ser_tx = AsyncCodec::decode::<cita::Transaction>(tx_data).await?;
        let message = ser_tx.hash();

        let private_key = C::PrivateKey::from_bytes(&privkey).unwrap();
        let signature = self.crypto.sign(&message, &private_key).unwrap();
        let untx = cita::UnverifiedTransaction {
            transaction: Some(ser_tx),
            signature:   signature.as_bytes().to_vec(),
            crypto:      0,
        };

        let ser_raw_tx = AsyncCodec::encode(untx).await?;
        let r = self.send_raw_transaction(ser_raw_tx).await?;
        Ok(r)
    }
}

/// A set of functions for FilterDataBase.
impl<E, T, S, C, P> AppState<E, T, S, C, P>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
    C: Crypto,
    P: PeerCount,
{
    /// Pass a block into FilterDatabase.
    pub async fn recv_block(&mut self, block: Block) -> RpcResult<()> {
        let mut ftdb = self.filterdb.write().compat().await.unwrap();
        ftdb.recv_block(block.clone());

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut uninstall_id = vec![];
        // If there are any log filter matchs the logs in block, insert the
        // logs into their dataset.panic!
        for (id, filter) in &ftdb.regs.clone() {
            // It's similar with RPC API `getLogs`, but has something different.

            // Check if we should uninstall the filter
            let lastupdate = ftdb.lastupdate.get(id).expect("must exist");
            if now - lastupdate > 60 {
                uninstall_id.push(*id);
                continue;
            }

            // Maybe we can save the result instead of the filter,
            // but at now, I want make it simply.
            let possible_blooms = filter.bloom_possibilities();
            let from_block = self.get_height(filter.from_block.clone()).await?;
            let to_block = self.get_height(filter.to_block.clone()).await?;

            if block.header.height < from_block || block.header.height > to_block {
                continue;
            }

            let mut logs: Vec<cita::Log> = vec![];
            let mut log_index = 0;

            let mut fit = false;
            for bloom in &possible_blooms {
                if block
                    .header
                    .logs_bloom
                    .contains_bloom(BloomRef::from(bloom))
                {
                    fit = true;
                    break;
                }
            }

            if !fit {
                continue;
            }

            let receipts_res = self
                .storage
                .get_receipts(Context::new(), block.tx_hashes.as_slice())
                .await?;

            for (tx_idx, tx_hash) in block.tx_hashes.iter().enumerate() {
                let receipt = &receipts_res[tx_idx];
                for (log_entry_index, log_entry) in receipt.logs.iter().enumerate() {
                    if filter.matches(&log_entry) {
                        let log = cita::Log {
                            address:               log_entry.address.clone(),
                            topics:                log_entry.topics.clone(),
                            data:                  cita::Data::from(log_entry.data.clone()),
                            block_hash:            Some(block.header.hash()),
                            block_number:          Some(block.header.height.into()),
                            transaction_hash:      Some(tx_hash.clone()),
                            transaction_index:     Some((tx_idx as u64).into()),
                            log_index:             Some(Uint::from(log_index as u64)),
                            transaction_log_index: Some((log_entry_index as u64).into()),
                        };
                        logs.push(log);
                    }
                    log_index += 1;
                }
            }

            let filter_logs = ftdb.data.entry(*id).or_insert_with(|| vec![]);
            filter_logs.extend(logs);
        }

        for id in uninstall_id {
            ftdb.uninstall(id);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_block_single() {
        common_logger::init(common_logger::Flag::Test);
        let mut fd = FilterDatabase::default();
        let id = fd.new_block_filter(100);

        let mut block = Block::default();
        block.header.height = 101;
        fd.recv_block(block);
        assert_eq!(fd.block_data.get(&id).unwrap().len(), 1);
    }

    #[test]
    fn test_filter_block_multiple() {
        common_logger::init(common_logger::Flag::Test);
        let mut fd = FilterDatabase::default();
        let id = fd.new_block_filter(100);

        let mut block = Block::default();

        block.header.height = 101;
        fd.recv_block(block.clone());
        block.header.height = 102;
        fd.recv_block(block.clone());
        block.header.height = 103;
        fd.recv_block(block.clone());

        assert_eq!(fd.block_data.get(&id).unwrap().len(), 3);
    }

    #[test]
    fn test_filter_block_and_then_fetch() {
        common_logger::init(common_logger::Flag::Test);
        let mut fd = FilterDatabase::default();
        let id = fd.new_block_filter(100);

        let mut block = Block::default();

        block.header.height = 101;
        fd.recv_block(block.clone());
        block.header.height = 102;
        fd.recv_block(block.clone());
        block.header.height = 103;
        fd.recv_block(block.clone());

        assert_eq!(fd.block_data.get(&id).unwrap().len(), 3);

        let changes = fd.filter_changes(id).unwrap();

        if let cita::FilterChanges::Hashes(hashed) = changes {
            assert_eq!(hashed.len(), 3);
        } else {
            panic!("The type of changes must be FilterChanges::Hashes")
        }

        let changes = fd.filter_changes(id).unwrap();
        if let cita::FilterChanges::Hashes(hashed) = changes {
            assert_eq!(hashed.len(), 0);
        } else {
            panic!("The type of changes must be FilterChanges::Hashes")
        }

        block.header.height = 104;
        fd.recv_block(block.clone());

        let changes = fd.filter_changes(id).unwrap();
        if let cita::FilterChanges::Hashes(hashed) = changes {
            assert_eq!(hashed.len(), 1);
        } else {
            panic!("The type of changes must be FilterChanges::Hashes")
        }
    }
}
