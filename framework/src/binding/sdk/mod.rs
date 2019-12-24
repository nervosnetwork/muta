mod chain_querier;

pub use chain_querier::{ChainQueryError, DefaultChainQuerier};

use std::cell::RefCell;
use std::rc::Rc;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    ChainQuerier, RequestContext, ServiceSDK, ServiceState, StoreArray, StoreBool, StoreMap,
    StoreString, StoreUint64,
};
use protocol::types::{Address, Epoch, Hash, Receipt, SignedTransaction};
use protocol::ProtocolResult;

use crate::binding::store::{
    DefaultStoreArray, DefaultStoreBool, DefaultStoreMap, DefaultStoreString, DefaultStoreUint64,
};

pub struct DefalutServiceSDK<S: ServiceState, C: ChainQuerier, R: RequestContext> {
    state:           Rc<RefCell<S>>,
    chain_querier:   Rc<C>,
    request_context: R,
}

impl<S: ServiceState, C: ChainQuerier, R: RequestContext> DefalutServiceSDK<S, C, R> {
    pub fn new(state: Rc<RefCell<S>>, chain_querier: Rc<C>, request_context: R) -> Self {
        Self {
            state,
            chain_querier,
            request_context,
        }
    }
}

impl<S: 'static + ServiceState, C: ChainQuerier, R: RequestContext> ServiceSDK
    for DefalutServiceSDK<S, C, R>
{
    type ContextItem = R;

    // Alloc or recover a `Map` by` var_name`
    fn alloc_or_recover_map<K: 'static + FixedCodec + PartialEq, V: 'static + FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreMap<K, V>>> {
        Ok(Box::new(DefaultStoreMap::<S, K, V>::new(
            Rc::clone(&self.state),
            var_name,
        )))
    }

    // Alloc or recover a `Array` by` var_name`
    fn alloc_or_recover_array<E: 'static + FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreArray<E>>> {
        Ok(Box::new(DefaultStoreArray::<S, E>::new(
            Rc::clone(&self.state),
            var_name,
        )))
    }

    // Alloc or recover a `Uint64` by` var_name`
    fn alloc_or_recover_uint64(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreUint64>> {
        Ok(Box::new(DefaultStoreUint64::new(
            Rc::clone(&self.state),
            var_name,
        )))
    }

    // Alloc or recover a `String` by` var_name`
    fn alloc_or_recover_string(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreString>> {
        Ok(Box::new(DefaultStoreString::new(
            Rc::clone(&self.state),
            var_name,
        )))
    }

    // Alloc or recover a `Bool` by` var_name`
    fn alloc_or_recover_bool(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreBool>> {
        Ok(Box::new(DefaultStoreBool::new(
            Rc::clone(&self.state),
            var_name,
        )))
    }

    // Get a value from the service state by key
    fn get_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        key: &Key,
    ) -> ProtocolResult<Option<Ret>> {
        self.state.borrow().get(key)
    }

    // Set a value to the service state by key
    fn set_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        key: Key,
        val: Val,
    ) -> ProtocolResult<()> {
        self.state.borrow_mut().insert(key, val)
    }

    // Get a value from the specified address by key
    fn get_account_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        address: &Address,
        key: &Key,
    ) -> ProtocolResult<Option<Ret>> {
        self.state.borrow().get_account_value(address, key)
    }

    // Insert a pair of key / value to the specified address
    fn set_account_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        address: &Address,
        key: Key,
        val: Val,
    ) -> ProtocolResult<()> {
        self.state.borrow_mut().set_account_value(address, key, val)
    }

    // Get a signed transaction by `tx_hash`
    // if not found on the chain, return None
    fn get_transaction_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<SignedTransaction>> {
        self.chain_querier.get_transaction_by_hash(tx_hash)
    }

    // Get a epoch by `epoch_id`
    // if not found on the chain, return None
    // When the parameter `epoch_id` is None, get the latest (executing)` epoch`
    fn get_epoch_by_epoch_id(&self, epoch_id: Option<u64>) -> ProtocolResult<Option<Epoch>> {
        self.chain_querier.get_epoch_by_epoch_id(epoch_id)
    }

    // Get a receipt by `tx_hash`
    // if not found on the chain, return None
    fn get_receipt_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<Receipt>> {
        self.chain_querier.get_receipt_by_hash(tx_hash)
    }

    fn get_request_context(&self) -> ProtocolResult<Self::ContextItem> {
        Ok(self.request_context.clone())
    }

    // Call other read-only methods of `service` and return the results
    // synchronously NOTE: You can use recursive calls, but the maximum call
    // stack is 1024
    fn read(&self, service: &str, method: &str, payload: &str) -> ProtocolResult<&str> {
        unimplemented!();
    }

    // Call other writable methods of `service` and return the results synchronously
    // NOTE: You can use recursive calls, but the maximum call stack is 1024
    fn write(&mut self, service: &str, method: &str, payload: &str) -> ProtocolResult<&str> {
        unimplemented!();
    }
}
