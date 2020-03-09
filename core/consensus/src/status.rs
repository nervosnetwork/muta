use std::sync::Arc;

use derive_more::Display;
use moodyblues_sdk::trace;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;

use common_merkle::Merkle;
use protocol::fixed_codec::FixedCodec;
use protocol::traits::ExecutorResp;
use protocol::types::{Block, Bloom, Hash, MerkleRoot, Metadata, Proof, Validator};

use crate::util::check_list_roots;

#[derive(Clone, Debug)]
pub struct StatusAgent {
    status: Arc<RwLock<CurrentConsensusStatus>>,
}

impl StatusAgent {
    pub fn new(status: CurrentConsensusStatus) -> Self {
        Self {
            status: Arc::new(RwLock::new(status)),
        }
    }

    pub fn update_by_executed(&self, info: ExecutedInfo) {
        self.status.write().update_by_executed(info);
    }

    pub fn update_by_commited(
        &self,
        metadata: Metadata,
        block: Block,
        block_hash: Hash,
        current_proof: Proof,
    ) {
        self.status
            .write()
            .update_by_commited(metadata, block, block_hash, current_proof)
    }

    // TODO(yejiayu): Is there a better way to write it?
    pub fn replace(&self, new_status: CurrentConsensusStatus) {
        let mut status = self.status.write();
        status.cycles_price = new_status.cycles_price;
        status.cycles_limit = new_status.cycles_limit;
        status.current_height = new_status.current_height;
        status.exec_height = new_status.exec_height;
        status.current_hash = new_status.current_hash;
        status.list_logs_bloom = new_status.list_logs_bloom;
        status.list_confirm_root = new_status.list_confirm_root;
        status.latest_state_root = new_status.latest_state_root;
        status.list_state_root = new_status.list_state_root;
        status.list_receipt_root = new_status.list_receipt_root;
        status.list_cycles_used = new_status.list_cycles_used;
        status.current_proof = new_status.current_proof;
        status.validators = new_status.validators;
        status.consensus_interval = new_status.consensus_interval;
    }

    pub fn to_inner(&self) -> CurrentConsensusStatus {
        self.status.read().clone()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Display)]
#[rustfmt::skip]
#[display(
    fmt = "current_height {}, exec height {}, current_hash {:?}, latest_state_root {:?} list state root {:?}, list receipt root {:?}, list confirm root {:?}, list cycle used {:?}, logs bloom {:?}",
    current_height, exec_height, current_hash, latest_state_root, list_state_root, list_receipt_root, list_confirm_root,
    list_cycles_used, "list_logs_bloom.iter().map(|bloom| bloom.to_low_u64_be()).collect::<Vec<_>>()"
)]
pub struct CurrentConsensusStatus {
    pub cycles_price:       u64,
    pub cycles_limit:       u64,
    pub current_height:     u64,
    pub exec_height:        u64,
    pub current_hash:       Hash,
    pub latest_state_root:  MerkleRoot,
    pub list_logs_bloom:    Vec<Bloom>,
    pub list_confirm_root:  Vec<MerkleRoot>,
    pub list_state_root:    Vec<MerkleRoot>,
    pub list_receipt_root:  Vec<MerkleRoot>,
    pub list_cycles_used:   Vec<u64>,
    pub current_proof:      Proof,
    pub validators:         Vec<Validator>,
    pub consensus_interval: u64,
    pub propose_ratio:      u64,
    pub prevote_ratio:      u64,
    pub precommit_ratio:    u64,
    pub brake_ratio:        u64,
    pub tx_num_limit:       u64,
    pub max_tx_size:        u64,
}

impl CurrentConsensusStatus {
    pub fn update_by_executed(&mut self, info: ExecutedInfo) {
        if info.exec_height <= self.exec_height {
            return;
        }
        log::info!("update_by_executed: info {}", info,);
        log::info!("update_by_executed: current status {}", self);
        // trace_after_exec(&info);

        assert!(info.exec_height == self.exec_height + 1);
        self.exec_height += 1;
        self.latest_state_root = info.state_root.clone();
        self.list_cycles_used.push(info.cycles_used);
        self.list_confirm_root.push(info.confirm_root.clone());
        self.list_logs_bloom.push(info.logs_bloom.clone());
        self.list_receipt_root.push(info.receipt_root.clone());
        self.list_state_root.push(info.state_root);
    }

    pub fn update_by_commited(
        &mut self,
        metadata: Metadata,
        block: Block,
        block_hash: Hash,
        current_proof: Proof,
    ) {
        log::info!(
            "update_by_commited: block {:?}, hash {:?}, state root {:?}",
            block.header,
            block_hash,
            block.header.state_root,
        );

        log::info!("update_by_commited: current status {}", self);

        self.set_metadata(metadata);

        assert!(block.header.height == self.current_height + 1);
        self.current_height = block.header.height;
        self.current_hash = block_hash;
        self.current_proof = current_proof;

        self.split_off(&block);
    }

    fn set_metadata(&mut self, metadata: Metadata) {
        self.cycles_limit = metadata.cycles_limit;
        self.cycles_price = metadata.cycles_price;
        self.consensus_interval = metadata.interval;
        let validators: Vec<Validator> = metadata
            .verifier_list
            .iter()
            .map(|v| Validator {
                address:        v.address.clone(),
                propose_weight: v.propose_weight,
                vote_weight:    v.vote_weight,
            })
            .collect();
        self.validators = validators;
        self.propose_ratio = metadata.propose_ratio;
        self.prevote_ratio = metadata.prevote_ratio;
        self.precommit_ratio = metadata.precommit_ratio;
    }

    fn split_off(&mut self, block: &Block) {
        let len = block.header.confirm_root.len();
        if len != block.header.cycles_used.len()
            || len != block.header.logs_bloom.len()
            || len != block.header.receipt_root.len()
        {
            panic!("vec lengths do not match. {:?}", block);
        }

        if !check_list_roots(&self.list_cycles_used, &block.header.cycles_used) {
            panic!(
                "check list_cycles_used error current_roots: {:?}, commited_roots roots {:?}",
                self.list_cycles_used, block.header.cycles_used
            );
        }
        if !check_list_roots(&self.list_logs_bloom, &block.header.logs_bloom) {
            panic!(
                "check list_logs_bloom error current_roots: {:?}, commited_roots roots {:?}",
                self.list_logs_bloom, block.header.logs_bloom
            );
        }
        if !check_list_roots(&self.list_confirm_root, &block.header.confirm_root) {
            panic!(
                "check list_confirm_root error current_roots: {:?}, commited_roots roots {:?}",
                self.list_confirm_root, block.header.confirm_root
            );
        }
        if !check_list_roots(&self.list_receipt_root, &block.header.receipt_root) {
            panic!(
                "check list_receipt_root error current_roots: {:?}, commited_roots roots {:?}",
                self.list_receipt_root, block.header.receipt_root
            );
        }

        self.list_cycles_used = self.list_cycles_used.split_off(len);
        self.list_logs_bloom = self.list_logs_bloom.split_off(len);
        self.list_confirm_root = self.list_confirm_root.split_off(len);
        self.list_receipt_root = self.list_receipt_root.split_off(len);
        self.list_state_root = self.list_state_root.split_off(len);
    }
}

#[derive(Clone, Debug, Display)]
#[rustfmt::skip]
#[display(
    fmt = "exec height {}, cycles used {}, state root {:?}, receipt root {:?}, confirm root {:?}, logs bloom {}",
    exec_height, cycles_used, state_root, receipt_root, confirm_root, "logs_bloom.to_low_u64_be()"
)]
pub struct ExecutedInfo {
    pub exec_height: u64,
    pub cycles_used:   u64,
    pub logs_bloom:    Bloom,
    pub state_root:    MerkleRoot,
    pub receipt_root:  MerkleRoot,
    pub confirm_root:  MerkleRoot,
}

impl ExecutedInfo {
    pub fn new(height: u64, order_root: MerkleRoot, resp: ExecutorResp) -> Self {
        let cycles = resp.all_cycles_used;

        let receipt = Merkle::from_hashes(
            resp.receipts
                .iter()
                .map(|r| Hash::digest(r.to_owned().encode_fixed().unwrap()))
                .collect::<Vec<_>>(),
        )
        .get_root_hash()
        .unwrap_or_else(Hash::from_empty);

        Self {
            exec_height:  height,
            cycles_used:  cycles,
            receipt_root: receipt,
            confirm_root: order_root,
            state_root:   resp.state_root.clone(),
            logs_bloom:   resp.logs_bloom,
        }
    }
}

pub fn trace_after_exec(info: &ExecutedInfo) {
    trace::custom(
        "update_by_executed".to_string(),
        Some(json!({
            "exec_height": info.exec_height,
            "state_root": info.state_root.as_hex(),
            "receipt_root": info.receipt_root.as_hex(),
            "confirm_root": info.confirm_root.as_hex(),
        })),
    );
}
