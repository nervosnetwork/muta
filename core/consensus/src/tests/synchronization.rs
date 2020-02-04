use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use futures::executor::block_on;
use futures::lock::Mutex;
use parking_lot::RwLock;

use common_merkle::Merkle;
use protocol::fixed_codec::FixedCodec;
use protocol::traits::{CommonConsensusAdapter, Synchronization, SynchronizationAdapter};
use protocol::traits::{Context, ExecutorParams, ExecutorResp};
use protocol::types::{
    Address, Block, BlockHeader, Bytes, Hash, MerkleRoot, Metadata, Proof, RawTransaction, Receipt,
    ReceiptResponse, SignedTransaction, TransactionRequest, Validator,
};
use protocol::ProtocolResult;

use crate::status::{CurrentConsensusStatus, StatusAgent};
use crate::synchronization::{OverlordSynchronization, RichBlock};

// Test the blocks gap from 1 to 10.
#[test]
fn sync_gap_test() {
    for gap in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10].iter() {
        let max_height = 77 * *gap;

        let list_rich_block = mock_chained_rich_block(max_height, *gap);

        let remote_blocks = gen_remote_block_hashmap(list_rich_block.clone());
        let genesis_block = remote_blocks.read().get(&0).unwrap().clone();

        let loacl_blocks = Arc::new(RwLock::new(HashMap::new()));
        loacl_blocks
            .write()
            .insert(genesis_block.header.height, genesis_block.clone());

        let local_transactions = Arc::new(RwLock::new(HashMap::new()));
        let remote_transactions = gen_remote_tx_hashmap(list_rich_block);

        let adapter = Arc::new(MockCommonConsensusAdapter::new(
            0,
            loacl_blocks,
            remote_blocks,
            local_transactions,
            remote_transactions,
        ));
        let status = CurrentConsensusStatus {
            cycles_price:       1,
            cycles_limit:       300_000_000,
            height:             genesis_block.header.height + 1,
            exec_height:        genesis_block.header.exec_height,
            prev_hash:          genesis_block.header.pre_hash,
            logs_bloom:         vec![],
            confirm_root:       vec![],
            latest_state_root:  genesis_block.header.state_root.clone(),
            state_root:         vec![],
            receipt_root:       vec![],
            cycles_used:        vec![],
            proof:              genesis_block.header.proof,
            validators:         genesis_block.header.validators,
            consensus_interval: 3000,
            propose_ratio:      15,
            prevote_ratio:      10,
            precommit_ratio:    10,
        };
        let status_agent = StatusAgent::new(status);
        let lock = Arc::new(Mutex::new(()));
        let sync = OverlordSynchronization::new(Arc::clone(&adapter), status_agent.clone(), lock);
        block_on(sync.receive_remote_block(Context::new(), max_height / 2)).unwrap();

        let status = status_agent.to_inner();
        let block =
            block_on(adapter.get_block_by_height(Context::new(), status.height - 1)).unwrap();
        assert_sync(status, block);

        block_on(sync.receive_remote_block(Context::new(), max_height)).unwrap();
        let status = status_agent.to_inner();
        let block =
            block_on(adapter.get_block_by_height(Context::new(), status.height - 1)).unwrap();
        assert_sync(status, block);
    }
}

pub type SafeHashMap<K, V> = Arc<RwLock<HashMap<K, V>>>;

pub struct MockCommonConsensusAdapter {
    latest_height:       RwLock<u64>,
    loacl_blocks:        SafeHashMap<u64, Block>,
    remote_blocks:       SafeHashMap<u64, Block>,
    local_transactions:  SafeHashMap<Hash, SignedTransaction>,
    remote_transactions: SafeHashMap<Hash, SignedTransaction>,
}

impl MockCommonConsensusAdapter {
    pub fn new(
        latest_height: u64,
        loacl_blocks: SafeHashMap<u64, Block>,
        remote_blocks: SafeHashMap<u64, Block>,
        local_transactions: SafeHashMap<Hash, SignedTransaction>,
        remote_transactions: SafeHashMap<Hash, SignedTransaction>,
    ) -> Self {
        Self {
            latest_height: RwLock::new(latest_height),
            loacl_blocks,
            remote_blocks,
            local_transactions,
            remote_transactions,
        }
    }
}

#[async_trait]
impl SynchronizationAdapter for MockCommonConsensusAdapter {
    fn update_status(
        &self,
        _: Context,
        _: u64,
        _: u64,
        _: u64,
        _: u64,
        _: u64,
        _: Vec<Validator>,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    fn sync_exec(
        &self,
        _: Context,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp> {
        Ok(exec_txs(params.height, txs).0)
    }

    /// Pull some blocks from other nodes from `begin` to `end`.
    async fn get_block_from_remote(&self, _: Context, height: u64) -> ProtocolResult<Block> {
        Ok(self.remote_blocks.read().get(&height).unwrap().clone())
    }

    /// Pull signed transactions corresponding to the given hashes from other
    /// nodes.
    async fn get_txs_from_remote(
        &self,
        _: Context,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let map = self.remote_transactions.read();
        let mut txs = vec![];

        for hash in tx_hashes.iter() {
            let tx = map.get(hash).unwrap();
            txs.push(tx.clone())
        }

        Ok(txs)
    }
}

#[async_trait]
impl CommonConsensusAdapter for MockCommonConsensusAdapter {
    /// Save a block to the database.
    async fn save_block(&self, _: Context, block: Block) -> ProtocolResult<()> {
        self.loacl_blocks.write().insert(block.header.height, block);
        let mut height = self.latest_height.write();
        *height += 1;
        Ok(())
    }

    async fn save_proof(&self, _: Context, _: Proof) -> ProtocolResult<()> {
        Ok(())
    }

    /// Save some signed transactions to the database.
    async fn save_signed_txs(
        &self,
        _: Context,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let mut map = self.local_transactions.write();
        for tx in signed_txs.into_iter() {
            map.insert(tx.tx_hash.clone(), tx);
        }
        Ok(())
    }

    async fn save_receipts(&self, _: Context, _: Vec<Receipt>) -> ProtocolResult<()> {
        Ok(())
    }

    /// Flush the given transactions in the mempool.
    async fn flush_mempool(&self, _: Context, _: &[Hash]) -> ProtocolResult<()> {
        Ok(())
    }

    /// Get a block corresponding to the given height.
    async fn get_block_by_height(&self, _: Context, height: u64) -> ProtocolResult<Block> {
        Ok(self.loacl_blocks.read().get(&height).unwrap().clone())
    }

    /// Get the current height from storage.
    async fn get_current_height(&self, _: Context) -> ProtocolResult<u64> {
        Ok(*self.latest_height.read())
    }

    async fn get_txs_from_storage(
        &self,
        _: Context,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let map = self.local_transactions.read();
        let mut txs = vec![];

        for hash in tx_hashes.iter() {
            let tx = map.get(hash).unwrap();
            txs.push(tx.clone())
        }

        Ok(txs)
    }

    async fn broadcast_height(&self, _: Context, _: u64) -> ProtocolResult<()> {
        Ok(())
    }

    fn get_metadata(
        &self,
        _context: Context,
        _state_root: MerkleRoot,
        _height: u64,
        _timestamp: u64,
    ) -> ProtocolResult<Metadata> {
        Ok(Metadata {
            chain_id:        Hash::from_empty(),
            common_ref:      "703873635a6b51513451".to_string(),
            timeout_gap:     20,
            cycles_limit:    9999,
            cycles_price:    1,
            interval:        3000,
            verifier_list:   vec![Validator {
                address:        Address::from_hex("1c9776983b2f251fa5c9cc562c1b667d1f05ff83")
                    .unwrap(),
                propose_weight: 0,
                vote_weight:    0,
            }],
            propose_ratio:   10,
            prevote_ratio:   10,
            precommit_ratio: 10,
        })
    }

    /// Set timeout_gap in mempool.
    fn set_timeout_gap(&self, _context: Context, _timeout_gap: u64) {}
}

fn gen_remote_tx_hashmap(list: Vec<RichBlock>) -> SafeHashMap<Hash, SignedTransaction> {
    let mut remote_txs = HashMap::new();

    for rich_block in list.into_iter() {
        for tx in rich_block.txs {
            remote_txs.insert(tx.tx_hash.clone(), tx);
        }
    }

    Arc::new(RwLock::new(remote_txs))
}

fn gen_remote_block_hashmap(list: Vec<RichBlock>) -> SafeHashMap<u64, Block> {
    let mut remote_blocks = HashMap::new();
    for rich_block in list.into_iter() {
        remote_blocks.insert(rich_block.block.header.height, rich_block.block.clone());
    }

    Arc::new(RwLock::new(remote_blocks))
}

fn mock_chained_rich_block(len: u64, gap: u64) -> Vec<RichBlock> {
    let mut list = vec![];

    let genesis_rich_block = mock_genesis_rich_block();
    list.push(genesis_rich_block.clone());

    let mut last_rich_block = genesis_rich_block;

    let mut current_height = 1;

    let mut temp_rich_block: Vec<RichBlock> = vec![];
    loop {
        let last_block_hash = Hash::digest(last_rich_block.block.encode_fixed().unwrap());
        let last_header = &last_rich_block.block.header;

        let txs = mock_tx_list(10, current_height);
        let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        let order_root = Merkle::from_hashes(tx_hashes.clone())
            .get_root_hash()
            .unwrap();

        let mut header = BlockHeader {
            chain_id: last_header.chain_id.clone(),
            height: current_height,
            exec_height: current_height,
            pre_hash: last_block_hash,
            timestamp: 0,
            order_root,
            logs_bloom: vec![],
            confirm_root: vec![],
            state_root: Hash::from_empty(),
            receipt_root: vec![],
            cycles_used: vec![],
            proposer: Address::from_hex("1c9776983b2f251fa5c9cc562c1b667d1f05ff83").unwrap(),
            proof: Proof {
                height:     current_height,
                round:      0,
                block_hash: Hash::from_empty(),
                signature:  Bytes::new(),
                bitmap:     Bytes::new(),
            },
            validator_version: 0,
            validators: vec![Validator {
                address:        Address::from_hex("1c9776983b2f251fa5c9cc562c1b667d1f05ff83")
                    .unwrap(),
                propose_weight: 0,
                vote_weight:    0,
            }],
        };

        if last_header.height != 0 && current_height % gap == 0 {
            temp_rich_block.iter().for_each(|rich_block| {
                let height = rich_block.block.header.height;
                let confirm_root = rich_block.block.header.order_root.clone();
                let (exec_resp, receipt_root) = exec_txs(height, &rich_block.txs);

                header.exec_height = height;
                header.logs_bloom.push(exec_resp.logs_bloom);
                header.confirm_root.push(confirm_root);
                header.state_root = exec_resp.state_root;
                header.receipt_root.push(receipt_root);
                header.cycles_used.push(exec_resp.all_cycles_used);
            });

            temp_rich_block.clear();
        } else if last_header.height != 0 && header.height != 1 {
            header.exec_height -= temp_rich_block.len() as u64 + 1;
        } else if header.height == 1 {
            header.exec_height -= 1;
        }

        let block = Block {
            header,
            ordered_tx_hashes: tx_hashes,
        };

        let rich_block = RichBlock { block, txs };

        list.push(rich_block.clone());
        temp_rich_block.push(rich_block.clone());
        last_rich_block = rich_block;
        current_height += 1;

        if current_height > len {
            break;
        }
    }

    list
}

fn mock_genesis_rich_block() -> RichBlock {
    let header = BlockHeader {
        chain_id:          Hash::from_empty(),
        height:            0,
        exec_height:       0,
        pre_hash:          Hash::from_empty(),
        timestamp:         0,
        logs_bloom:        vec![],
        order_root:        Hash::from_empty(),
        confirm_root:      vec![],
        state_root:        Hash::from_empty(),
        receipt_root:      vec![],
        cycles_used:       vec![],
        proposer:          Address::from_hex("1c9776983b2f251fa5c9cc562c1b667d1f05ff83").unwrap(),
        proof:             Proof {
            height:     0,
            round:      0,
            block_hash: Hash::from_empty(),
            signature:  Bytes::new(),
            bitmap:     Bytes::new(),
        },
        validator_version: 0,
        validators:        vec![Validator {
            address:        Address::from_hex("1c9776983b2f251fa5c9cc562c1b667d1f05ff83").unwrap(),
            propose_weight: 0,
            vote_weight:    0,
        }],
    };
    let genesis_block = Block {
        header,
        ordered_tx_hashes: vec![],
    };

    RichBlock {
        block: genesis_block,
        txs:   vec![],
    }
}

fn get_receipt(tx: &SignedTransaction, height: u64) -> Receipt {
    Receipt {
        state_root: MerkleRoot::from_empty(),
        height,
        tx_hash: tx.tx_hash.clone(),
        cycles_used: tx.raw.cycles_limit,
        events: vec![],
        response: ReceiptResponse {
            service_name: "sync".to_owned(),
            method:       "sync_exec".to_owned(),
            ret:          "".to_owned(),
            is_error:     false,
        },
    }
}

fn mock_tx_list(num: usize, height: u64) -> Vec<SignedTransaction> {
    let mut txs = vec![];

    for i in 0..num {
        let raw = RawTransaction {
            chain_id:     Hash::from_empty(),
            nonce:        Hash::digest(Bytes::from(format!("{}", i))),
            timeout:      height,
            cycles_price: 1,
            cycles_limit: 1,
            request:      TransactionRequest {
                service_name: "test".to_owned(),
                method:       "test".to_owned(),
                payload:      "test".to_owned(),
            },
        };

        let bytes = raw.encode_fixed().unwrap();
        let signed_tx = SignedTransaction {
            raw,
            tx_hash: Hash::digest(bytes),
            pubkey: Bytes::new(),
            signature: Bytes::new(),
        };

        txs.push(signed_tx)
    }

    txs
}

fn exec_txs(height: u64, txs: &[SignedTransaction]) -> (ExecutorResp, MerkleRoot) {
    let mut receipts = vec![];
    let mut all_cycles_used = 0;

    for tx in txs.iter() {
        let receipt = get_receipt(tx, height);
        all_cycles_used += receipt.cycles_used;
        receipts.push(receipt);
    }
    let receipt_root = Merkle::from_hashes(
        receipts
            .iter()
            .map(|r| Hash::digest(r.to_owned().encode_fixed().unwrap()))
            .collect::<Vec<_>>(),
    )
    .get_root_hash()
    .unwrap_or_else(Hash::from_empty);

    (
        ExecutorResp {
            receipts,
            all_cycles_used,
            logs_bloom: Default::default(),
            state_root: MerkleRoot::from_empty(),
        },
        receipt_root,
    )
}

fn assert_sync(status: CurrentConsensusStatus, latest_block: Block) {
    let exec_gap = latest_block.header.height - latest_block.header.exec_height;

    assert_eq!(status.height - 1, latest_block.header.height);
    assert_eq!(status.exec_height, latest_block.header.height);
    assert_eq!(status.confirm_root.len(), exec_gap as usize);
    assert_eq!(status.cycles_used.len(), exec_gap as usize);
    assert_eq!(status.logs_bloom.len(), exec_gap as usize);
    assert_eq!(status.receipt_root.len(), exec_gap as usize);
}
