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
    Address, Bytes, Epoch, EpochHeader, Hash, MerkleRoot, Proof, RawTransaction, Receipt,
    ReceiptResponse, SignedTransaction, TransactionRequest, Validator,
};
use protocol::ProtocolResult;

use crate::status::{CurrentConsensusStatus, StatusAgent};
use crate::synchronization_v2::{OverlordSynchronization, RichEpoch};

// Test the epochs gap from 1 to 10.
#[test]
fn sync_gap_test() {
    for gap in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10].iter() {
        let max_epoch_id = 77 * *gap;

        let list_rich_epoch = mock_chained_rich_epoch(max_epoch_id, *gap);

        let remote_epochs = gen_remote_epoch_hashmap(list_rich_epoch.clone());
        let genesis_epoch = remote_epochs.read().get(&0).unwrap().clone();

        let local_epochs = Arc::new(RwLock::new(HashMap::new()));
        local_epochs
            .write()
            .insert(genesis_epoch.header.epoch_id, genesis_epoch.clone());

        let local_transactions = Arc::new(RwLock::new(HashMap::new()));
        let remote_transactions = gen_remote_tx_hashmap(list_rich_epoch);

        let adapter = Arc::new(MockCommonConsensusAdapter::new(
            0,
            local_epochs,
            remote_epochs,
            local_transactions,
            remote_transactions,
        ));
        let status = CurrentConsensusStatus {
            cycles_price:       1,
            cycles_limit:       300_000_000,
            epoch_id:           genesis_epoch.header.epoch_id,
            exec_epoch_id:      genesis_epoch.header.exec_epoch_id,
            prev_hash:          genesis_epoch.header.pre_hash,
            logs_bloom:         vec![],
            confirm_root:       vec![],
            latest_state_root:  genesis_epoch.header.state_root.clone(),
            state_root:         vec![],
            receipt_root:       vec![],
            cycles_used:        vec![],
            proof:              genesis_epoch.header.proof,
            validators:         genesis_epoch.header.validators,
            consensus_interval: 3000,
        };
        let status_agent = StatusAgent::new(status);
        let lock = Arc::new(Mutex::new(()));
        let sync = OverlordSynchronization::new(Arc::clone(&adapter), status_agent.clone(), lock);
        block_on(sync.receive_remote_epoch(Context::new(), max_epoch_id / 2)).unwrap();

        let status = status_agent.to_inner();
        let epoch = block_on(adapter.get_epoch_by_id(Context::new(), status.epoch_id)).unwrap();
        assert_sync(status, epoch);

        block_on(sync.receive_remote_epoch(Context::new(), max_epoch_id)).unwrap();
        let status = status_agent.to_inner();
        let epoch = block_on(adapter.get_epoch_by_id(Context::new(), status.epoch_id)).unwrap();
        assert_sync(status, epoch);
    }
}

pub type SafeHashMap<K, V> = Arc<RwLock<HashMap<K, V>>>;

pub struct MockCommonConsensusAdapter {
    latest_epoch_id:     RwLock<u64>,
    local_epochs:        SafeHashMap<u64, Epoch>,
    remote_epochs:       SafeHashMap<u64, Epoch>,
    local_transactions:  SafeHashMap<Hash, SignedTransaction>,
    remote_transactions: SafeHashMap<Hash, SignedTransaction>,
}

impl MockCommonConsensusAdapter {
    pub fn new(
        latest_epoch_id: u64,
        local_epochs: SafeHashMap<u64, Epoch>,
        remote_epochs: SafeHashMap<u64, Epoch>,
        local_transactions: SafeHashMap<Hash, SignedTransaction>,
        remote_transactions: SafeHashMap<Hash, SignedTransaction>,
    ) -> Self {
        Self {
            latest_epoch_id: RwLock::new(latest_epoch_id),
            local_epochs,
            remote_epochs,
            local_transactions,
            remote_transactions,
        }
    }
}

#[async_trait]
impl SynchronizationAdapter for MockCommonConsensusAdapter {
    fn sync_exec(
        &self,
        _: Context,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp> {
        Ok(exec_txs(params.epoch_id, txs).0)
    }

    /// Pull some epochs from other nodes from `begin` to `end`.
    async fn get_epoch_from_remote(&self, _: Context, epoch_id: u64) -> ProtocolResult<Epoch> {
        Ok(self.remote_epochs.read().get(&epoch_id).unwrap().clone())
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
    /// Save an epoch to the database.
    async fn save_epoch(&self, _: Context, epoch: Epoch) -> ProtocolResult<()> {
        self.local_epochs
            .write()
            .insert(epoch.header.epoch_id, epoch);
        let mut epoch_id = self.latest_epoch_id.write();
        *epoch_id += 1;
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

    /// Get an epoch corresponding to the given epoch ID.
    async fn get_epoch_by_id(&self, _: Context, epoch_id: u64) -> ProtocolResult<Epoch> {
        Ok(self.local_epochs.read().get(&epoch_id).unwrap().clone())
    }

    /// Get the current epoch ID from storage.
    async fn get_current_epoch_id(&self, _: Context) -> ProtocolResult<u64> {
        Ok(*self.latest_epoch_id.read())
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

    async fn broadcast_epoch_id(&self, _: Context, _: u64) -> ProtocolResult<()> {
        Ok(())
    }
}

fn gen_remote_tx_hashmap(list: Vec<RichEpoch>) -> SafeHashMap<Hash, SignedTransaction> {
    let mut remote_txs = HashMap::new();

    for rich_epoch in list.into_iter() {
        for tx in rich_epoch.txs {
            remote_txs.insert(tx.tx_hash.clone(), tx);
        }
    }

    Arc::new(RwLock::new(remote_txs))
}

fn gen_remote_epoch_hashmap(list: Vec<RichEpoch>) -> SafeHashMap<u64, Epoch> {
    let mut remote_epochs = HashMap::new();
    for rich_epoch in list.into_iter() {
        remote_epochs.insert(rich_epoch.epoch.header.epoch_id, rich_epoch.epoch.clone());
    }

    Arc::new(RwLock::new(remote_epochs))
}

fn mock_chained_rich_epoch(len: u64, gap: u64) -> Vec<RichEpoch> {
    let mut list = vec![];

    let genesis_rich_epoch = mock_genesis_rich_epoch();
    list.push(genesis_rich_epoch.clone());

    let mut last_rich_epoch = genesis_rich_epoch;

    let mut current_epoch_id = 1;

    let mut temp_rich_epoch: Vec<RichEpoch> = vec![];
    loop {
        let last_epoch_hash = Hash::digest(last_rich_epoch.epoch.encode_fixed().unwrap());
        let last_header = &last_rich_epoch.epoch.header;

        let txs = mock_tx_list(10, current_epoch_id);
        let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        let order_root = Merkle::from_hashes(tx_hashes.clone())
            .get_root_hash()
            .unwrap();

        let mut header = EpochHeader {
            chain_id: last_header.chain_id.clone(),
            epoch_id: current_epoch_id,
            exec_epoch_id: current_epoch_id,
            pre_hash: last_epoch_hash,
            timestamp: 0,
            order_root,
            logs_bloom: vec![],
            confirm_root: vec![],
            state_root: Hash::from_empty(),
            receipt_root: vec![],
            cycles_used: vec![],
            proposer: Address::from_hex("1c9776983b2f251fa5c9cc562c1b667d1f05ff83").unwrap(),
            proof: Proof {
                epoch_id:   current_epoch_id,
                round:      0,
                epoch_hash: Hash::from_empty(),
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

        if last_header.epoch_id != 0 && current_epoch_id % gap == 0 {
            temp_rich_epoch.iter().for_each(|rich_epoch| {
                let epoch_id = rich_epoch.epoch.header.epoch_id;
                let confirm_root = rich_epoch.epoch.header.order_root.clone();
                let (exec_resp, receipt_root) = exec_txs(epoch_id, &rich_epoch.txs);

                header.exec_epoch_id = epoch_id;
                header.logs_bloom.push(exec_resp.logs_bloom);
                header.confirm_root.push(confirm_root);
                header.state_root = exec_resp.state_root;
                header.receipt_root.push(receipt_root);
                header.cycles_used.push(exec_resp.all_cycles_used);
            });

            temp_rich_epoch.clear();
        } else if last_header.epoch_id != 0 && header.epoch_id != 1 {
            header.exec_epoch_id -= temp_rich_epoch.len() as u64 + 1;
        } else if header.epoch_id == 1 {
            header.exec_epoch_id -= 1;
        }

        let epoch = Epoch {
            header,
            ordered_tx_hashes: tx_hashes,
        };

        let rich_epoch = RichEpoch { epoch, txs };

        list.push(rich_epoch.clone());
        temp_rich_epoch.push(rich_epoch.clone());
        last_rich_epoch = rich_epoch;
        current_epoch_id += 1;

        if current_epoch_id > len {
            break;
        }
    }

    list
}

fn mock_genesis_rich_epoch() -> RichEpoch {
    let header = EpochHeader {
        chain_id:          Hash::from_empty(),
        epoch_id:          0,
        exec_epoch_id:     0,
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
            epoch_id:   0,
            round:      0,
            epoch_hash: Hash::from_empty(),
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
    let genesis_epoch = Epoch {
        header,
        ordered_tx_hashes: vec![],
    };

    RichEpoch {
        epoch: genesis_epoch,
        txs:   vec![],
    }
}

fn get_receipt(tx: &SignedTransaction, epoch_id: u64) -> Receipt {
    Receipt {
        state_root: MerkleRoot::from_empty(),
        epoch_id,
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

fn mock_tx_list(num: usize, epoch_id: u64) -> Vec<SignedTransaction> {
    let mut txs = vec![];

    for i in 0..num {
        let raw = RawTransaction {
            chain_id:     Hash::from_empty(),
            nonce:        Hash::digest(Bytes::from(format!("{}", i))),
            timeout:      epoch_id,
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

fn exec_txs(epoch_id: u64, txs: &[SignedTransaction]) -> (ExecutorResp, MerkleRoot) {
    let mut receipts = vec![];
    let mut all_cycles_used = 0;

    for tx in txs.iter() {
        let receipt = get_receipt(tx, epoch_id);
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

fn assert_sync(status: CurrentConsensusStatus, latest_epoch: Epoch) {
    let exec_gap = latest_epoch.header.epoch_id - latest_epoch.header.exec_epoch_id;

    assert_eq!(status.epoch_id, latest_epoch.header.epoch_id);
    assert_eq!(status.exec_epoch_id, latest_epoch.header.epoch_id);
    assert_eq!(status.confirm_root.len(), exec_gap as usize);
    assert_eq!(status.cycles_used.len(), exec_gap as usize);
    assert_eq!(status.logs_bloom.len(), exec_gap as usize);
    assert_eq!(status.receipt_root.len(), exec_gap as usize);
}
