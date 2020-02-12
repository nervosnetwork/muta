use std::sync::Arc;

use derive_more::Display;
use log::{error, info};
use moodyblues_sdk::trace;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;

use common_merkle::Merkle;
use protocol::fixed_codec::FixedCodec;
use protocol::traits::ExecutorResp;
use protocol::types::{Block, Bloom, Hash, MerkleRoot, Metadata, Proof, Validator};
use protocol::ProtocolResult;

use crate::engine::check_vec_roots;
use crate::{ConsensusError, StatusCacheField};

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

    pub fn update_after_exec(&self, info: UpdateInfo) {
        self.status.write().update_after_exec(info);
    }

    pub fn update_after_commit(
        &self,
        height: u64,
        metadata: Metadata,
        block: Block,
        prev_hash: Hash,
        proof: Proof,
    ) -> ProtocolResult<()> {
        self.status
            .write()
            .update_after_commit(height, metadata, block, prev_hash, proof)
    }

    pub fn update_after_sync_commit(
        &self,
        height: u64,
        metadata: Metadata,
        block: Block,
        prev_hash: Hash,
        proof: Proof,
    ) {
        self.status
            .write()
            .update_after_sync_commit(height, metadata, block, prev_hash, proof)
    }

    // TODO(yejiayu): Is there a better way to write it?
    pub fn replace(&self, new_status: CurrentConsensusStatus) {
        let mut status = self.status.write();
        status.cycles_price = new_status.cycles_price;
        status.cycles_limit = new_status.cycles_limit;
        status.height = new_status.height;
        status.exec_height = new_status.exec_height;
        status.prev_hash = new_status.prev_hash;
        status.logs_bloom = new_status.logs_bloom;
        status.confirm_root = new_status.confirm_root;
        status.latest_state_root = new_status.latest_state_root;
        status.state_root = new_status.state_root;
        status.receipt_root = new_status.receipt_root;
        status.cycles_used = new_status.cycles_used;
        status.proof = new_status.proof;
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
    fmt = "height {}, exec height {}, prev_hash {:?},latest_state_root {:?} state root {:?}, receipt root {:?}, confirm root {:?}, cycle used {:?}, logs bloom {:?}",
    height, exec_height, prev_hash, latest_state_root, state_root, receipt_root, confirm_root,
    cycles_used, "logs_bloom.iter().map(|bloom| bloom.to_low_u64_be()).collect::<Vec<_>>()"
)]
pub struct CurrentConsensusStatus {
    pub cycles_price:       u64,
    pub cycles_limit:       u64,
    pub height:             u64,
    pub exec_height:        u64,
    pub prev_hash:          Hash,
    pub latest_state_root:  MerkleRoot,
    pub logs_bloom:         Vec<Bloom>,
    pub confirm_root:       Vec<MerkleRoot>,
    pub state_root:         Vec<MerkleRoot>,
    pub receipt_root:       Vec<MerkleRoot>,
    pub cycles_used:        Vec<u64>,
    pub proof:              Proof,
    pub validators:         Vec<Validator>,
    pub consensus_interval: u64,
    pub propose_ratio:      u64,
    pub prevote_ratio:      u64,
    pub precommit_ratio:    u64,
    pub brake_ratio:        u64,
}

impl CurrentConsensusStatus {
    pub fn update_after_exec(&mut self, info: UpdateInfo) {
        info!("update_after_exec info {}", info);
        info!("update_after_exec cache: {}", self);
        trace_after_exec(&info);

        assert!(info.exec_height == self.exec_height + 1);
        self.exec_height += 1;
        self.latest_state_root = info.state_root.clone();
        self.cycles_used.push(info.cycles_used);
        self.confirm_root.push(info.confirm_root.clone());
        self.logs_bloom.push(info.logs_bloom.clone());
        self.receipt_root.push(info.receipt_root.clone());

        if self.state_root.last() != Some(&info.state_root) {
            self.state_root.push(info.state_root);
        }
    }

    pub fn update_after_commit(
        &mut self,
        height: u64,
        metadata: Metadata,
        block: Block,
        prev_hash: Hash,
        proof: Proof,
    ) -> ProtocolResult<()> {
        info!(
            "update info {}, prev hash {:?}, state root {:?}",
            height, prev_hash, block.header.state_root
        );
        info!("update after commit cache: {}", self);

        self.set_metadata(metadata);

        self.height = height;
        self.prev_hash = prev_hash;
        self.proof = proof;

        self.update_cycles(&block.header.cycles_used)?;
        self.update_logs_bloom(&block.header.logs_bloom)?;
        self.update_state_root(&block.header.state_root)?;
        self.update_confirm_root(&block.header.confirm_root)?;
        self.update_receipt_root(&block.header.receipt_root)?;
        Ok(())
    }

    pub fn update_after_sync_commit(
        &mut self,
        height: u64,
        metadata: Metadata,
        block: Block,
        prev_hash: Hash,
        proof: Proof,
    ) {
        self.set_metadata(metadata);

        self.height = height;
        self.prev_hash = prev_hash;
        self.proof = proof;

        self.cycles_used = self.cycles_used.split_off(block.header.cycles_used.len());
        self.logs_bloom = self.logs_bloom.split_off(block.header.logs_bloom.len());
        self.confirm_root = self.confirm_root.split_off(block.header.confirm_root.len());
        self.receipt_root = self.receipt_root.split_off(block.header.receipt_root.len());
    }

    fn set_metadata(&mut self, metadata: Metadata) {
        self.cycles_limit = metadata.cycles_limit;
        self.cycles_price = metadata.cycles_price;
        self.consensus_interval = metadata.interval;
        self.validators = metadata.verifier_list;
        self.propose_ratio = metadata.propose_ratio;
        self.prevote_ratio = metadata.prevote_ratio;
        self.precommit_ratio = metadata.precommit_ratio;
    }

    fn update_cycles(&mut self, cycles: &[u64]) -> ProtocolResult<()> {
        if !check_vec_roots(&self.cycles_used, cycles) {
            error!(
                "block cycles used {:?}, cache cycles used {:?}",
                cycles, self.cycles_used
            );
            return Err(ConsensusError::StatusErr(StatusCacheField::CyclesUsed).into());
        }

        self.cycles_used = self.cycles_used.split_off(cycles.len());
        Ok(())
    }

    fn update_logs_bloom(&mut self, logs: &[Bloom]) -> ProtocolResult<()> {
        if !check_vec_roots(&self.logs_bloom, logs) {
            error!(
                "block cycles used {:?}, cache cycles used {:?}",
                logs.iter()
                    .map(|bloom| bloom.to_low_u64_be())
                    .collect::<Vec<_>>(),
                self.logs_bloom
                    .iter()
                    .map(|bloom| bloom.to_low_u64_be())
                    .collect::<Vec<_>>(),
            );
            return Err(ConsensusError::StatusErr(StatusCacheField::LogsBloom).into());
        }

        self.logs_bloom = self.logs_bloom.split_off(logs.len());
        Ok(())
    }

    fn update_state_root(&mut self, state_root: &MerkleRoot) -> ProtocolResult<()> {
        if self.state_root.is_empty() {
            return Ok(());
        } else if self.state_root.len() == 1 {
            if state_root != self.state_root.get(0).unwrap() {
                return Err(ConsensusError::StatusErr(StatusCacheField::StateRoot).into());
            }
            return Ok(());
        }

        let mut at = usize::max_value();
        for (index, item) in self.state_root.iter().enumerate() {
            if item == state_root {
                at = index;
                break;
            }
        }

        if at == usize::max_value() {
            error!("state root: {:?}", state_root);
            return Err(ConsensusError::StatusErr(StatusCacheField::StateRoot).into());
        }

        let tmp = self.state_root.split_off(at);
        self.state_root = tmp;
        Ok(())
    }

    fn update_receipt_root(&mut self, receipt_roots: &[MerkleRoot]) -> ProtocolResult<()> {
        if !check_vec_roots(&self.receipt_root, receipt_roots) {
            error!(
                "block receipt root: {:?}, cache receipt roots {:?}",
                receipt_roots, self.receipt_root
            );
            return Err(ConsensusError::StatusErr(StatusCacheField::ReceiptRoot).into());
        }

        self.receipt_root = self.receipt_root.split_off(receipt_roots.len());
        Ok(())
    }

    fn update_confirm_root(&mut self, confirm_root: &[MerkleRoot]) -> ProtocolResult<()> {
        if confirm_root.is_empty() {
            return Ok(());
        }

        let len = confirm_root.len();
        if self.confirm_root.len() < len || self.confirm_root[len - 1] != confirm_root[len - 1] {
            error!(
                "block confirm root: {:?}, cache confirm roots {:?}",
                confirm_root, self.confirm_root
            );
            return Err(ConsensusError::StatusErr(StatusCacheField::ConfirmRoot).into());
        }

        let tmp = self.confirm_root.split_off(len);
        self.confirm_root = tmp;
        Ok(())
    }
}

#[derive(Clone, Debug, Display)]
#[rustfmt::skip]
#[display(
    fmt = "exec height {}, cycles used {}, state root {:?}, receipt root {:?}, confirm root {:?}, logs bloom {}",
    exec_height, cycles_used, state_root, receipt_root, confirm_root, "logs_bloom.to_low_u64_be()"
)]
pub struct UpdateInfo {
    pub exec_height: u64,
    pub cycles_used:   u64,
    pub logs_bloom:    Bloom,
    pub state_root:    MerkleRoot,
    pub receipt_root:  MerkleRoot,
    pub confirm_root:  MerkleRoot,
}

impl UpdateInfo {
    pub fn with_after_exec(height: u64, order_root: MerkleRoot, resp: ExecutorResp) -> Self {
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

pub fn trace_after_exec(info: &UpdateInfo) {
    trace::custom(
        "update_exec_info".to_string(),
        Some(json!({
            "exec_height": info.exec_height,
            "state_root": info.state_root.as_hex(),
            "receipt_root": info.receipt_root.as_hex(),
            "confirm_root": info.confirm_root.as_hex(),
        })),
    );
}
