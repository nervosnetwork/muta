use std::sync::Arc;

use derive_more::Display;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use common_merkle::Merkle;
use protocol::fixed_codec::FixedCodec;
use protocol::traits::{Context, ExecutorResp};
use protocol::types::{Block, Hash, MerkleRoot, Metadata, Proof, Validator};

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

    pub fn update_by_committed(
        &self,
        metadata: Metadata,
        block: Block,
        block_hash: Hash,
        current_proof: Proof,
    ) {
        self.status
            .write()
            .update_by_committed(metadata, block, block_hash, current_proof)
    }

    // TODO(yejiayu): Is there a better way to write it?
    pub fn replace(&self, new_status: CurrentConsensusStatus) {
        let mut status = self.status.write();
        status.cycles_price = new_status.cycles_price;
        status.cycles_limit = new_status.cycles_limit;
        status.latest_committed_height = new_status.latest_committed_height;
        status.exec_height = new_status.exec_height;
        status.current_hash = new_status.current_hash;
        status.latest_committed_state_root = new_status.latest_committed_state_root;
        status.list_confirm_root = new_status.list_confirm_root;
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

#[derive(Serialize, Deserialize, Clone, Debug, Display, PartialEq, Eq)]
#[display(
    fmt = "latest_committed_height {}, exec height {}, current_hash {:?}, latest_committed_state_root {:?} list state root {:?}, list receipt root {:?}, list confirm root {:?}, list cycle used {:?}",
    latest_committed_height,
    exec_height,
    current_hash,
    latest_committed_state_root,
    list_state_root,
    list_receipt_root,
    list_confirm_root,
    list_cycles_used
)]
pub struct CurrentConsensusStatus {
    pub cycles_price:                u64, // metadata
    pub cycles_limit:                u64, // metadata
    pub latest_committed_height:     u64, // latest consented height
    pub exec_height:                 u64,
    pub current_hash:                Hash, // as same as block of current height
    pub latest_committed_state_root: MerkleRoot, // latest consented height
    pub list_confirm_root:           Vec<MerkleRoot>,
    pub list_state_root:             Vec<MerkleRoot>,
    pub list_receipt_root:           Vec<MerkleRoot>,
    pub list_cycles_used:            Vec<u64>,
    pub current_proof:               Proof, // latest consented block's proof, not previous block
    pub validators:                  Vec<Validator>, // metadate
    pub consensus_interval:          u64,   // metadata
    pub propose_ratio:               u64,   // metadata
    pub prevote_ratio:               u64,   // metadata
    pub precommit_ratio:             u64,   // metadata
    pub brake_ratio:                 u64,
    pub tx_num_limit:                u64,
    pub max_tx_size:                 u64,
} // metadata is as same as latest consented height

impl CurrentConsensusStatus {
    pub fn get_latest_state_root(&self) -> MerkleRoot {
        self.list_state_root
            .last()
            .unwrap_or(&self.latest_committed_state_root)
            .clone()
    }

    pub(crate) fn update_by_executed(&mut self, info: ExecutedInfo) {
        if info.exec_height <= self.exec_height {
            return;
        }
        log::info!("update_by_executed: info {}", info,);
        log::info!("update_by_executed: current status {}", self);

        assert!(info.exec_height == self.exec_height + 1);
        self.exec_height += 1;
        self.list_cycles_used.push(info.cycles_used);
        self.list_confirm_root.push(info.confirm_root.clone());
        self.list_receipt_root.push(info.receipt_root.clone());
        self.list_state_root.push(info.state_root);
    }

    pub(crate) fn update_by_committed(
        &mut self,
        metadata: Metadata,
        block: Block,
        block_hash: Hash,
        current_proof: Proof,
    ) {
        self.set_metadata(metadata);

        assert!(block.header.height == self.latest_committed_height + 1);

        self.latest_committed_height = block.header.height;
        self.current_hash = block_hash;
        self.current_proof = current_proof;
        self.latest_committed_state_root = block.header.state_root.clone();

        self.split_off(&block);
    }

    pub(crate) fn set_metadata(&mut self, metadata: Metadata) {
        self.cycles_limit = metadata.cycles_limit;
        self.cycles_price = metadata.cycles_price;
        self.consensus_interval = metadata.interval;
        let validators: Vec<Validator> = metadata
            .verifier_list
            .iter()
            .map(|v| Validator {
                pub_key:        v.pub_key.decode(),
                address:        v.address.as_bytes(),
                propose_weight: v.propose_weight,
                vote_weight:    v.vote_weight,
            })
            .collect();
        self.validators = validators;
        self.propose_ratio = metadata.propose_ratio;
        self.prevote_ratio = metadata.prevote_ratio;
        self.precommit_ratio = metadata.precommit_ratio;
        self.brake_ratio = metadata.brake_ratio;
        self.max_tx_size = metadata.max_tx_size;
        self.tx_num_limit = metadata.tx_num_limit;
    }

    fn split_off(&mut self, block: &Block) {
        let len = block.header.confirm_root.len();
        if len != block.header.cycles_used.len() || len != block.header.receipt_root.len() {
            panic!("vec lengths do not match. {:?}", block);
        }

        if !check_list_roots(&self.list_cycles_used, &block.header.cycles_used) {
            panic!(
                "check list_cycles_used error current_roots: {:?}, committed_roots roots {:?}",
                self.list_cycles_used, block.header.cycles_used
            );
        }
        if !check_list_roots(&self.list_confirm_root, &block.header.confirm_root) {
            panic!(
                "check list_confirm_root error current_roots: {:?}, committed_roots roots {:?}",
                self.list_confirm_root, block.header.confirm_root
            );
        }
        if !check_list_roots(&self.list_receipt_root, &block.header.receipt_root) {
            panic!(
                "check list_receipt_root error current_roots: {:?}, committed_roots roots {:?}",
                self.list_receipt_root, block.header.receipt_root
            );
        }

        self.list_cycles_used = self.list_cycles_used.split_off(len);
        self.list_confirm_root = self.list_confirm_root.split_off(len);
        self.list_receipt_root = self.list_receipt_root.split_off(len);
        self.list_state_root = self.list_state_root.split_off(len);
    }
}

#[derive(Clone, Debug, Display)]
#[display(
    fmt = "exec height {}, cycles used {}, state root {:?}, receipt root {:?}, confirm root {:?}",
    exec_height,
    cycles_used,
    state_root,
    receipt_root,
    confirm_root
)]
pub struct ExecutedInfo {
    pub ctx:          Context,
    pub exec_height:  u64,
    pub cycles_used:  u64,
    pub state_root:   MerkleRoot,
    pub receipt_root: MerkleRoot,
    pub confirm_root: MerkleRoot,
}

impl ExecutedInfo {
    pub fn new(ctx: Context, height: u64, order_root: MerkleRoot, resp: ExecutorResp) -> Self {
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
            ctx,
            exec_height: height,
            cycles_used: cycles,
            receipt_root: receipt,
            confirm_root: order_root,
            state_root: resp.state_root,
        }
    }
}
