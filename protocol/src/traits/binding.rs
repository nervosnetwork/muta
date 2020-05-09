use std::collections::BTreeMap;
use std::iter::Iterator;

use bytes::Bytes;

use crate::fixed_codec::FixedCodec;
use crate::traits::{ExecutorParams, ServiceResponse};
use crate::types::{
    Address, Block, DataMeta, Hash, Hex, MerkleRoot, Receipt, ScalarMeta, ServiceContext,
    ServiceMeta, SignedTransaction,
};
use crate::ProtocolResult;

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
    fn genesis_(&mut self, _payload: String) {}

    // Called before block execution
    fn hook_before_(&mut self, _params: &ExecutorParams) {}

    // Called after block execution
    fn hook_after_(&mut self, _params: &ExecutorParams) {}

    // Called before tx execution
    fn tx_hook_before_(&mut self, _ctx: ServiceContext) {}
    // Called after tx execution
    fn tx_hook_after_(&mut self, _ctx: ServiceContext) {}

    fn write_(&mut self, ctx: ServiceContext) -> ServiceResponse<String>;

    fn read_(&self, ctx: ServiceContext) -> ServiceResponse<String>;

    // Return service schema: (MethodSchema, EventSchema)
    fn meta_(&self) -> ServiceMeta;
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
    ) -> Box<dyn StoreMap<Key, Val>>;

    // Alloc or recover a `Array` by` var_name`
    fn alloc_or_recover_array<Elm: 'static + FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> Box<dyn StoreArray<Elm>>;

    // Alloc or recover a `Uint64` by` var_name`
    fn alloc_or_recover_uint64(&mut self, var_name: &str) -> Box<dyn StoreUint64>;

    // Alloc or recover a `String` by` var_name`
    fn alloc_or_recover_string(&mut self, var_name: &str) -> Box<dyn StoreString>;

    // Alloc or recover a `Bool` by` var_name`
    fn alloc_or_recover_bool(&mut self, var_name: &str) -> Box<dyn StoreBool>;

    // Get a value from the service state by key
    fn get_value<Key: FixedCodec, Ret: FixedCodec>(&self, key: &Key) -> Option<Ret>;

    // Set a value to the service state by key
    fn set_value<Key: FixedCodec, Val: FixedCodec>(&mut self, key: Key, val: Val);

    // Get a value from the specified address by key
    fn get_account_value<Key: FixedCodec, Ret: FixedCodec>(
        &self,
        address: &Address,
        key: &Key,
    ) -> Option<Ret>;

    // Insert a pair of key / value to the specified address
    fn set_account_value<Key: FixedCodec, Val: FixedCodec>(
        &mut self,
        address: &Address,
        key: Key,
        val: Val,
    );

    // Get a signed transaction by `tx_hash`
    // if not found on the chain, return None
    fn get_transaction_by_hash(&self, tx_hash: &Hash) -> Option<SignedTransaction>;

    // Get a block by `height`
    // if not found on the chain, return None
    // When the parameter `height` is None, get the latest (executing)` block`
    fn get_block_by_height(&self, height: Option<u64>) -> Option<Block>;

    // Get a receipt by `tx_hash`
    // if not found on the chain, return None
    fn get_receipt_by_hash(&self, tx_hash: &Hash) -> Option<Receipt>;

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
    ) -> ServiceResponse<String>;

    // Call other writable methods of `service` and return the results synchronously
    // NOTE: You can use recursive calls, but the maximum call stack is 1024
    fn write(
        &mut self,
        ctx: &ServiceContext,
        extra: Option<Bytes>,
        service: &str,
        method: &str,
        payload: &str,
    ) -> ServiceResponse<String>;
}

pub trait StoreMap<K: FixedCodec + PartialEq, V: FixedCodec> {
    fn get(&self, key: &K) -> Option<V>;

    fn contains(&self, key: &K) -> bool;

    fn insert(&mut self, key: K, value: V);

    fn remove(&mut self, key: &K) -> Option<V>;

    fn len(&self) -> u32;

    fn is_empty(&self) -> bool;

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (&K, V)> + 'a>;
}

pub trait StoreArray<E: FixedCodec> {
    fn get(&self, index: u32) -> Option<E>;

    fn push(&mut self, element: E);

    fn remove(&mut self, index: u32);

    fn len(&self) -> u32;

    fn is_empty(&self) -> bool;

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (u32, E)> + 'a>;
}

pub trait StoreUint64 {
    fn get(&self) -> u64;

    fn set(&mut self, val: u64);

    // Add val with self
    // And set the result back to self
    fn safe_add(&mut self, val: u64) -> bool;

    // Self minus val
    // And set the result back to self
    fn safe_sub(&mut self, val: u64) -> bool;

    // Multiply val with self
    // And set the result back to self
    fn safe_mul(&mut self, val: u64) -> bool;

    // Power of self
    // And set the result back to self
    fn safe_pow(&mut self, val: u32) -> bool;

    // Self divided by val
    // And set the result back to self
    fn safe_div(&mut self, val: u64) -> bool;

    // Remainder of self
    // And set the result back to self
    fn safe_rem(&mut self, val: u64) -> bool;
}

pub trait StoreString {
    fn get(&self) -> String;

    fn set(&mut self, val: &str);

    fn len(&self) -> u32;

    fn is_empty(&self) -> bool;
}

pub trait StoreBool {
    fn get(&self) -> bool;

    fn set(&mut self, b: bool);
}

pub trait MetaGenerator {
    fn name() -> String;
    fn meta(register: &mut BTreeMap<String, DataMeta>);
}

macro_rules! impl_scalar_meta {
    ($t: ident, $s: expr) => {
        impl MetaGenerator for $t {
            fn name() -> String {
                $s.to_owned()
            }

            fn meta(register: &mut BTreeMap<String, DataMeta>) {
                if "String" == $s || "Boolean" == $s {
                    return;
                }
                let meta = ScalarMeta {
                    name:    $s.to_owned(),
                    comment: "".to_owned(),
                };
                register.insert($s.to_string(), DataMeta::Scalar(meta));
            }
        }
    };
    ($t: ident, $s: expr, $d: expr) => {
        impl MetaGenerator for $t {
            fn name() -> String {
                $s.to_owned()
            }

            fn meta(register: &mut BTreeMap<String, DataMeta>) {
                if "String" == $s || "Boolean" == $s {
                    return;
                }
                let meta = ScalarMeta {
                    name:    $s.to_owned(),
                    comment: "# ".to_owned() + $d + "\n",
                };
                register.insert($s.to_string(), DataMeta::Scalar(meta));
            }
        }
    };
}

impl_scalar_meta![u8, "U8"];
impl_scalar_meta![u32, "U32"];
impl_scalar_meta![u64, "U64"];
impl_scalar_meta![bool, "Boolean"];
impl_scalar_meta![String, "String"];
impl_scalar_meta![Address, "Address", "20 bytes of account address"];
impl_scalar_meta![Hash, "Hash", "The output digest of Keccak hash function"];
impl_scalar_meta![Hex, "Hex", "String started with 0x"];
