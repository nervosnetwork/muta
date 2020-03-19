mod chain_querier;

pub use chain_querier::{ChainQueryError, DefaultChainQuerier};

use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;
use derive_more::{Display, From};

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    ChainQuerier, Dispatcher, ServiceResponse, ServiceSDK, ServiceState, StoreArray, StoreBool,
    StoreMap, StoreString, StoreUint64,
};
use protocol::types::{Address, Block, Hash, Receipt, ServiceContext, SignedTransaction};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::binding::store::{
    DefaultStoreArray, DefaultStoreBool, DefaultStoreMap, DefaultStoreString, DefaultStoreUint64,
};

pub struct DefalutServiceSDK<S: ServiceState, C: ChainQuerier, D: Dispatcher> {
    state:         Rc<RefCell<S>>,
    chain_querier: Rc<C>,
    dispatcher:    D,
}

impl<S: ServiceState, C: ChainQuerier, D: Dispatcher> DefalutServiceSDK<S, C, D> {
    pub fn new(state: Rc<RefCell<S>>, chain_querier: Rc<C>, dispatcher: D) -> Self {
        Self {
            state,
            chain_querier,
            dispatcher,
        }
    }
}

impl<S: 'static + ServiceState, C: ChainQuerier, D: Dispatcher> ServiceSDK
    for DefalutServiceSDK<S, C, D>
{
    // Alloc or recover a `Map` by` var_name`
    fn alloc_or_recover_map<K: 'static + FixedCodec + PartialEq, V: 'static + FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> Box<dyn StoreMap<K, V>> {
        Box::new(DefaultStoreMap::<S, K, V>::new(
            Rc::clone(&self.state),
            var_name,
        ))
    }

    // Alloc or recover a `Array` by` var_name`
    fn alloc_or_recover_array<E: 'static + FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> Box<dyn StoreArray<E>> {
        Box::new(DefaultStoreArray::<S, E>::new(
            Rc::clone(&self.state),
            var_name,
        ))
    }

    // Alloc or recover a `Uint64` by` var_name`
    fn alloc_or_recover_uint64(&mut self, var_name: &str) -> Box<dyn StoreUint64> {
        Box::new(DefaultStoreUint64::new(Rc::clone(&self.state), var_name))
    }

    // Alloc or recover a `String` by` var_name`
    fn alloc_or_recover_string(&mut self, var_name: &str) -> Box<dyn StoreString> {
        Box::new(DefaultStoreString::new(Rc::clone(&self.state), var_name))
    }

    // Alloc or recover a `Bool` by` var_name`
    fn alloc_or_recover_bool(&mut self, var_name: &str) -> Box<dyn StoreBool> {
        Box::new(DefaultStoreBool::new(Rc::clone(&self.state), var_name))
    }

    // Get a value from the service state by key
    fn get_value<Key: FixedCodec, Ret: FixedCodec>(&self, key: &Key) -> Option<Ret> {
        self.state
            .borrow()
            .get(key)
            .unwrap_or_else(|e| panic!("service sdk get value failed: {}", e))
    }

    // Set a value to the service state by key
    fn set_value<Key: FixedCodec, Val: FixedCodec>(&mut self, key: Key, val: Val) {
        self.state
            .borrow_mut()
            .insert(key, val)
            .unwrap_or_else(|e| panic!("service sdk set value failed: {}", e));
    }

    // Get a value from the specified address by key
    fn get_account_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        address: &Address,
        key: &Key,
    ) -> Option<Ret> {
        self.state
            .borrow()
            .get_account_value(address, key)
            .unwrap_or_else(|e| panic!("service sdk get account value failed: {}", e))
    }

    // Insert a pair of key / value to the specified address
    fn set_account_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        address: &Address,
        key: Key,
        val: Val,
    ) {
        self.state
            .borrow_mut()
            .set_account_value(address, key, val)
            .unwrap_or_else(|e| panic!("service sdk set account value failed: {}", e));
    }

    // Get a signed transaction by `tx_hash`
    // if not found on the chain, return None
    fn get_transaction_by_hash(&self, tx_hash: &Hash) -> Option<SignedTransaction> {
        self.chain_querier
            .get_transaction_by_hash(tx_hash)
            .unwrap_or_else(|e| panic!("service sdk get transaction by hash failed: {}", e))
    }

    // Get a block by `height`
    // if not found on the chain, return None
    // When the parameter `height` is None, get the latest (executing)` block`
    fn get_block_by_height(&self, height: Option<u64>) -> Option<Block> {
        self.chain_querier
            .get_block_by_height(height)
            .unwrap_or_else(|e| panic!("service sdk get block by height failed: {}", e))
    }

    // Get a receipt by `tx_hash`
    // if not found on the chain, return None
    fn get_receipt_by_hash(&self, tx_hash: &Hash) -> Option<Receipt> {
        self.chain_querier
            .get_receipt_by_hash(tx_hash)
            .unwrap_or_else(|e| panic!("service sdk get receipt by hash failed: {}", e))
    }

    // Call other read-only methods of `service` and return the results
    // synchronously NOTE: You can use recursive calls, but the maximum call
    // stack is 1024
    fn read(
        &self,
        ctx: &ServiceContext,
        extra: Option<Bytes>,
        service: &str,
        method: &str,
        payload: &str,
    ) -> ServiceResponse<String> {
        let ctx = ServiceContext::with_context(
            ctx,
            extra,
            service.to_string(),
            method.to_string(),
            payload.to_string(),
        );

        self.dispatcher.read(ctx)
    }

    // Call other writable methods of `service` and return the results synchronously
    // NOTE: You can use recursive calls, but the maximum call stack is 1024
    fn write(
        &mut self,
        ctx: &ServiceContext,
        extra: Option<Bytes>,
        service: &str,
        method: &str,
        payload: &str,
    ) -> ServiceResponse<String> {
        let ctx = ServiceContext::with_context(
            ctx,
            extra,
            service.to_string(),
            method.to_string(),
            payload.to_string(),
        );

        self.dispatcher.write(ctx)
    }
}

#[derive(Debug, Display, From)]
pub enum SDKError {
    #[display(fmt = "dispatch failed: {:?}", error)]
    DispatchFailed { error: String },
}
impl std::error::Error for SDKError {}

impl From<SDKError> for ProtocolError {
    fn from(err: SDKError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Binding, Box::new(err))
    }
}
