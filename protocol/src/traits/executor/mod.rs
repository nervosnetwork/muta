pub mod contract;

use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;

use crate::types::{
    Address, Bloom, CarryingAsset, ContractAddress, Fee, Hash, MerkleRoot, Receipt,
    SignedTransaction,
};
use crate::ProtocolResult;

#[derive(Clone, Debug)]
pub struct ExecutorExecResp {
    pub receipts:        Vec<Receipt>,
    pub all_cycles_used: Vec<Fee>,
    pub logs_bloom:      Bloom,
    pub state_root:      MerkleRoot,
}

pub trait Executor {
    fn exec(
        &mut self,
        epoch_id: u64,
        cycles_price: u64,
        coinbase: Address,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<ExecutorExecResp>;
}

#[derive(Clone, Debug)]
pub struct InvokeContext {
    pub chain_id:       Hash,
    pub cycles_used:    Fee,
    pub cycles_limit:   Fee,
    pub cycles_price:   u64,
    pub epoch_id:       u64,
    pub caller:         Address,
    pub carrying_asset: Option<CarryingAsset>,
    pub coinbase:       Address,
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
