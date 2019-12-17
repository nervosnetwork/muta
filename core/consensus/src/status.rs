use std::sync::Arc;

use derive_more::Display;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::stream::StreamExt;
use log::{error, warn};
use parking_lot::RwLock;

use protocol::types::{Bloom, Epoch, Hash, MerkleRoot, Proof, Validator};
use protocol::{ProtocolError, ProtocolResult};

use crate::{ConsensusError, StatusCacheField};

#[derive(Debug)]
pub struct StatusPivot {
    status: Arc<RwLock<CurrentConsensusStatus>>,
    msg_rx: UnboundedReceiver<UpdateInfo>,
}

impl StatusPivot {
    pub fn new(status: Arc<RwLock<CurrentConsensusStatus>>) -> (Self, CurrentStatusAgent) {
        let (tx, msg_rx) = unbounded();
        let pivot = StatusPivot { status, msg_rx };
        let agent = CurrentStatusAgent::new(tx);
        (pivot, agent)
    }

    pub async fn run(mut self) {
        loop {
            if let Some(msg) = self.msg_rx.next().await {
                self.update(msg);
            } else {
                error!("muta-consensus: Status agent disconnected");
            }
        }
    }

    fn update(&self, info: UpdateInfo) {
        let mut status = self.status.write();
        status.update_after_exec(info);
    }
}

#[derive(Clone, Debug)]
pub struct CurrentStatusAgent {
    tx: UnboundedSender<UpdateInfo>,
}

impl CurrentStatusAgent {
    pub fn new(tx: UnboundedSender<UpdateInfo>) -> Self {
        CurrentStatusAgent { tx }
    }

    pub fn send(&mut self, info: UpdateInfo) -> ProtocolResult<()> {
        self.tx.unbounded_send(info).map_err(|e| {
            ProtocolError::from(ConsensusError::Other(format!("Status agent error {:?}", e)))
        })?;
        Ok(())
    }
}

#[derive(Clone, Debug, Display)]
#[rustfmt::skip]
#[display(
    fmt = "epoch ID {}, exec epoch ID {}, prev_hash {:?}, state root {:?}, receipt root {:?}, confirm root {:?}, cycle used {:?}",
    epoch_id, exec_epoch_id, prev_hash, state_root, receipt_root, confirm_root, cycles_used
)]
pub struct CurrentConsensusStatus {
    pub cycles_price:       u64,
    pub cycles_limit:       u64,
    pub epoch_id:           u64,
    pub exec_epoch_id:      u64,
    pub prev_hash:          Hash,
    pub logs_bloom:         Vec<Bloom>,
    pub confirm_root:       Vec<MerkleRoot>,
    pub state_root:         Vec<MerkleRoot>,
    pub receipt_root:       Vec<MerkleRoot>,
    pub cycles_used:        Vec<u64>,
    pub proof:              Proof,
    pub validators:         Vec<Validator>,
    pub consensus_interval: u64,
}

impl CurrentConsensusStatus {
    fn update_after_exec(&mut self, info: UpdateInfo) {
        warn!("Update {}", info);
        warn!("AE: {}", self);

        assert!(info.exec_epoch_id == self.exec_epoch_id + 1);
        self.exec_epoch_id += 1;
        self.cycles_used.push(info.cycles_used);
        self.confirm_root.push(info.confirm_root.clone());
        self.logs_bloom.push(info.logs_bloom.clone());
        self.state_root.push(info.state_root.clone());
        self.receipt_root.push(info.receipt_root);
    }

    pub fn update_after_commit(
        &mut self,
        epoch_id: u64,
        epoch: Epoch,
        prev_hash: Hash,
        proof: Proof,
    ) -> ProtocolResult<()> {
        warn!("update {:?}, {:?}", epoch_id, prev_hash);
        warn!("AC: {}", self);

        self.epoch_id = epoch_id;
        self.prev_hash = prev_hash;
        self.proof = proof;

        self.update_cycles(&epoch.header.cycles_used)?;
        self.update_logs_bloom(&epoch.header.logs_bloom)?;
        self.update_state_root(&epoch.header.state_root)?;
        self.update_confirm_root(&epoch.header.confirm_root)?;
        self.update_receipt_root(&epoch.header.receipt_root)?;
        Ok(())
    }

    fn update_cycles(&mut self, cycles: &[u64]) -> ProtocolResult<()> {
        if cycles.is_empty() {
            return Ok(());
        }

        let len = cycles.len();
        if self.cycles_used.len() < len || self.cycles_used[len - 1] != cycles[len - 1] {
            return Err(ConsensusError::StatusErr(StatusCacheField::CyclesUsed).into());
        }

        let tmp = self.cycles_used.split_off(len);
        self.cycles_used = tmp;
        Ok(())
    }

    fn update_logs_bloom(&mut self, log: &[Bloom]) -> ProtocolResult<()> {
        if log.is_empty() {
            return Ok(());
        }

        let len = log.len();
        if self.logs_bloom.len() < len || self.logs_bloom[len - 1] != log[len - 1] {
            return Err(ConsensusError::StatusErr(StatusCacheField::LogsBloom).into());
        }

        let tmp = self.logs_bloom.split_off(len);
        self.logs_bloom = tmp;
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
            warn!("state root: {:?}", state_root);
            return Err(ConsensusError::StatusErr(StatusCacheField::StateRoot).into());
        } else if at == self.state_root.len() - 1 {
            at -= 1;
        }

        let tmp = self.state_root.split_off(at + 1);
        self.state_root = tmp;
        Ok(())
    }

    fn update_receipt_root(&mut self, receipt_root: &[MerkleRoot]) -> ProtocolResult<()> {
        if receipt_root.is_empty() {
            return Ok(());
        }

        let len = receipt_root.len();
        if self.receipt_root.len() < len || self.receipt_root[len - 1] != receipt_root[len - 1] {
            warn!("receipt root: {:?}", receipt_root);
            return Err(ConsensusError::StatusErr(StatusCacheField::ReceiptRoot).into());
        }

        let tmp = self.receipt_root.split_off(len);
        self.receipt_root = tmp;
        Ok(())
    }

    fn update_confirm_root(&mut self, confirm_root: &[MerkleRoot]) -> ProtocolResult<()> {
        if confirm_root.is_empty() {
            return Ok(());
        }

        let len = confirm_root.len();
        if self.confirm_root.len() < len || self.confirm_root[len - 1] != confirm_root[len - 1] {
            warn!("confirm root: {:?}", confirm_root);
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
    fmt = "exec epoch ID {}, cycles used {}, state root {:?}, receipt root {:?}, confirm root {:?}",
    exec_epoch_id, cycles_used, state_root, receipt_root, confirm_root
)]
pub struct UpdateInfo {
    pub exec_epoch_id: u64,
    pub cycles_used:   u64,
    pub logs_bloom:    Bloom,
    pub state_root:    MerkleRoot,
    pub receipt_root:  MerkleRoot,
    pub confirm_root:  MerkleRoot,
}
