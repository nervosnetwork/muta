use std::iter::Iterator;

use bytes::Bytes;
use derive_more::{Display, From};

use crate::fixed_codec::FixedCodec;
use crate::traits::ExecutorParams;
use crate::types::{Address, Block, Hash, MerkleRoot, Receipt, ServiceContext, SignedTransaction};
use crate::{ProtocolError, ProtocolErrorKind, ProtocolResult};

#[derive(Debug, Display, From)]
pub enum BindingMacroError {
    #[display(fmt = "service {:?} method {:?} was not found", service, method)]
    NotFoundMethod { service: String, method: String },

    #[display(fmt = "Parsing payload to json failed {:?}", _0)]
    JsonParse(serde_json::Error),
}
impl std::error::Error for BindingMacroError {}

impl From<BindingMacroError> for ProtocolError {
    fn from(err: BindingMacroError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::BindingMacro, Box::new(err))
    }
}

pub trait ServiceMapping: Send + Sync {
    fn get_service<SDK: 'static + ServiceSDK>(
        &self,
        name: &str,
        sdk: SDK,
    ) -> ProtocolResult<Box<dyn Service>>;

    fn list_service_name(&self) -> Vec<String>;
}

// `ServiceState` provides access to` world state` and `account` for` service`.
// The bottom layer is an MPT tree.
//
// Each `service` will have a separate` ServiceState`, so their states are
// isolated from each other.
pub trait ServiceState {
    fn get<Key: FixedCodec, Ret: FixedCodec>(&self, key: &Key) -> ProtocolResult<Option<Ret>>;

    fn contains<Key: FixedCodec>(&self, key: &Key) -> ProtocolResult<bool>;

    // Insert a pair of key / value
    // Note: This key/value pair will go into the cache first
    // and will not be persisted to MPT until `commit` is called.
    fn insert<Key: FixedCodec, Value: FixedCodec>(
        &mut self,
        key: Key,
        value: Value,
    ) -> ProtocolResult<()>;

    fn get_account_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        address: &Address,
        key: &Key,
    ) -> ProtocolResult<Option<Ret>>;

    fn set_account_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        address: &Address,
        key: Key,
        val: Val,
    ) -> ProtocolResult<()>;

    // Roll back all data in the cache
    fn revert_cache(&mut self) -> ProtocolResult<()>;

    // Move data from cache to stash
    fn stash(&mut self) -> ProtocolResult<()>;

    // Persist data from stash into MPT
    fn commit(&mut self) -> ProtocolResult<MerkleRoot>;
}

pub trait ChainQuerier {
    fn get_transaction_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<SignedTransaction>>;

    // To get the latest `Block` of finality, set `height` to `None`
    fn get_block_by_height(&self, height: Option<u64>) -> ProtocolResult<Option<Block>>;

    fn get_receipt_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<Receipt>>;
}

// Admission control will be called before entering service
pub trait AdmissionControl {
    fn next<SDK: ServiceSDK>(&self, ctx: ServiceContext, sdk: SDK) -> ProtocolResult<()>;
}

// Developers can use service to customize blockchain business
//
// It contains:
// - init: Initialize the service.
// - hooks: A pair of hooks that allow inserting a piece of logic before and
//   after the block is executed.
// - read: Provide some read-only functions for users or other services to call
// - write: provide some writable functions for users or other services to call
pub trait Service {
    // Executed to create genesis states when starting chain
    fn genesis_(&mut self, _payload: String) -> ProtocolResult<()> {
        Ok(())
    }

    // Executed before the block is executed.
    fn hook_before_(&mut self, _params: &ExecutorParams) -> ProtocolResult<()> {
        Ok(())
    }

    // Executed after block execution.
    fn hook_after_(&mut self, _params: &ExecutorParams) -> ProtocolResult<()> {
        Ok(())
    }

    fn write_(&mut self, ctx: ServiceContext) -> ProtocolResult<String>;

    fn read_(&self, ctx: ServiceContext) -> ProtocolResult<String>;
}

// `ServiceSDK` provides multiple rich interfaces for `service` developers
//
// It contains:
//
// - Various data structures that store data to `world state`(call
//   `alloc_or_recover_*`)
// - Access and modify `account`
// - Access service state
// - Event triggered
// - Access to data on the chain (block, transaction, receipt)
// - Read / write other `service`
//
// In fact, these functions depend on:
//
// - ChainDB
// - ServiceState
pub trait ServiceSDK {
    // Alloc or recover a `Map` by` var_name`
    fn alloc_or_recover_map<Key: 'static + FixedCodec + PartialEq, Val: 'static + FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreMap<Key, Val>>>;

    // Alloc or recover a `Array` by` var_name`
    fn alloc_or_recover_array<Elm: 'static + FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreArray<Elm>>>;

    // Alloc or recover a `Uint64` by` var_name`
    fn alloc_or_recover_uint64(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreUint64>>;

    // Alloc or recover a `String` by` var_name`
    fn alloc_or_recover_string(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreString>>;

    // Alloc or recover a `Bool` by` var_name`
    fn alloc_or_recover_bool(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreBool>>;

    // Get a value from the service state by key
    fn get_value<Key: FixedCodec, Ret: FixedCodec>(&self, key: &Key)
        -> ProtocolResult<Option<Ret>>;

    // Set a value to the service state by key
    fn set_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        key: Key,
        val: Val,
    ) -> ProtocolResult<()>;

    // Get a value from the specified address by key
    fn get_account_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        address: &Address,
        key: &Key,
    ) -> ProtocolResult<Option<Ret>>;

    // Insert a pair of key / value to the specified address
    fn set_account_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        address: &Address,
        key: Key,
        val: Val,
    ) -> ProtocolResult<()>;

    // Get a signed transaction by `tx_hash`
    // if not found on the chain, return None
    fn get_transaction_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<SignedTransaction>>;

    // Get a block by `height`
    // if not found on the chain, return None
    // When the parameter `height` is None, get the latest (executing)` block`
    fn get_block_by_height(&self, height: Option<u64>) -> ProtocolResult<Option<Block>>;

    // Get a receipt by `tx_hash`
    // if not found on the chain, return None
    fn get_receipt_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<Receipt>>;

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
    ) -> ProtocolResult<String>;

    // Call other writable methods of `service` and return the results synchronously
    // NOTE: You can use recursive calls, but the maximum call stack is 1024
    fn write(
        &mut self,
        ctx: &ServiceContext,
        extra: Option<Bytes>,
        service: &str,
        method: &str,
        payload: &str,
    ) -> ProtocolResult<String>;
}

pub trait StoreMap<Key: FixedCodec + PartialEq, Value: FixedCodec> {
    fn get(&self, key: &Key) -> ProtocolResult<Value>;

    fn contains(&self, key: &Key) -> ProtocolResult<bool>;

    fn insert(&mut self, key: Key, value: Value) -> ProtocolResult<()>;

    fn remove(&mut self, key: &Key) -> ProtocolResult<()>;

    fn len(&self) -> ProtocolResult<u32>;

    fn is_empty(&self) -> ProtocolResult<bool>;

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (&Key, Value)> + 'a>;
}

pub trait StoreArray<Elm: FixedCodec> {
    fn get(&self, index: u32) -> ProtocolResult<Elm>;

    fn push(&mut self, element: Elm) -> ProtocolResult<()>;

    fn remove(&mut self, index: u32) -> ProtocolResult<()>;

    fn len(&self) -> ProtocolResult<u32>;

    fn is_empty(&self) -> ProtocolResult<bool>;

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (u32, Elm)> + 'a>;
}

pub trait StoreUint64 {
    fn get(&self) -> ProtocolResult<u64>;

    fn set(&mut self, val: u64) -> ProtocolResult<()>;

    // Add val with self
    // And set the result back to self
    fn add(&mut self, val: u64) -> ProtocolResult<()>;

    // Self minus val
    // And set the result back to self
    fn sub(&mut self, val: u64) -> ProtocolResult<()>;

    // Multiply val with self
    // And set the result back to self
    fn mul(&mut self, val: u64) -> ProtocolResult<()>;

    // Power of self
    // And set the result back to self
    fn pow(&mut self, val: u32) -> ProtocolResult<()>;

    // Self divided by val
    // And set the result back to self
    fn div(&mut self, val: u64) -> ProtocolResult<()>;

    // Remainder of self
    // And set the result back to self
    fn rem(&mut self, val: u64) -> ProtocolResult<()>;
}

pub trait StoreString {
    fn get(&self) -> ProtocolResult<String>;

    fn set(&mut self, val: &str) -> ProtocolResult<()>;

    fn len(&self) -> ProtocolResult<u32>;

    fn is_empty(&self) -> ProtocolResult<bool>;
}

pub trait StoreBool {
    fn get(&self) -> ProtocolResult<bool>;

    fn set(&mut self, b: bool) -> ProtocolResult<()>;
}
