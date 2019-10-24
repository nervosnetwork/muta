pub mod contract;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use bytes::Bytes;

use crate::types::{
    Address, AssetID, Balance, Bloom, CarryingAsset, ContractAddress, Fee, Genesis, Hash,
    MerkleRoot, Receipt, SignedTransaction,
};
use crate::ProtocolResult;

pub trait TrieDB = cita_trie::DB;

#[derive(Clone, Debug)]
pub struct ExecutorExecResp {
    pub receipts:        Vec<Receipt>,
    pub all_cycles_used: Vec<Fee>,
    pub logs_bloom:      Bloom,
    pub state_root:      MerkleRoot,
}

pub trait ExecutorFactory<DB: TrieDB>: Send + Sync {
    fn from_root(
        chain_id: Hash,
        state_root: MerkleRoot,
        db: Arc<DB>,
        epoch_id: u64,
        cycles_price: u64,
        coinbase: Address,
    ) -> ProtocolResult<Box<dyn Executor>>;
}

pub trait Executor {
    fn create_genesis(&mut self, genesis: &Genesis) -> ProtocolResult<MerkleRoot>;

    fn exec(&mut self, signed_txs: Vec<SignedTransaction>) -> ProtocolResult<ExecutorExecResp>;

    fn get_balance(&self, address: &Address, id: &AssetID) -> ProtocolResult<Balance>;
}

#[derive(Clone, Debug)]
pub struct InvokeContext {
    pub chain_id:       Hash,
    pub cycles_used:    u64,
    pub cycles_limit:   u64,
    pub fee_asset_id:   AssetID,
    pub cycles_price:   u64,
    pub epoch_id:       u64,
    pub caller:         Address,
    pub coinbase:       Address,
    pub carrying_asset: Option<CarryingAsset>,
}

pub type RcInvokeContext = Rc<RefCell<InvokeContext>>;

pub trait Dispatcher {
    fn invoke(
        &self,
        ictx: RcInvokeContext,
        address: ContractAddress,
        method: &str,
        args: Vec<Bytes>,
    ) -> ProtocolResult<Bytes>;
}

pub trait ContractSchema {
    type Key: ContractSer + Clone + std::hash::Hash + PartialEq + Eq + PartialOrd + Ord;
    type Value: ContractSer + Clone;
}

pub trait ContractSer {
    fn encode(&self) -> ProtocolResult<Bytes>;

    fn decode(bytes: Bytes) -> ProtocolResult<Self>
    where
        Self: Sized;
}
