use crate::fixed_codec::FixedCodec;
use crate::types::{Address, Epoch, Hash, MerkleRoot, Receipt, SignedTransaction};
use crate::ProtocolResult;

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

pub trait ChainDB {
    fn get_transaction_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<SignedTransaction>>;

    fn get_epoch_by_epoch_id(&self, epoch_id: Option<u64>) -> ProtocolResult<Option<Epoch>>;

    fn get_receipt_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<Receipt>>;
}

pub trait RequestContext: Clone {
    fn sub_cycles(&self, cycels: u64) -> ProtocolResult<()>;

    fn get_cycles_price(&self) -> ProtocolResult<u64>;

    fn get_cycles_limit(&self) -> ProtocolResult<u64>;

    fn get_cycles_used(&self) -> ProtocolResult<u64>;

    fn get_caller(&self) -> ProtocolResult<Address>;

    fn get_current_epoch_id(&self) -> ProtocolResult<u64>;

    fn get_service_name(&self) -> ProtocolResult<&str>;

    fn get_service_method(&self) -> ProtocolResult<&str>;

    fn get_payload(&self) -> ProtocolResult<&str>;
}

// Admission control will be called before entering service
pub trait AdmissionControl {
    fn next<SDK: ServiceSDK, Context: RequestContext>(
        &self,
        ctx: Context,
        sdk: SDK,
    ) -> ProtocolResult<()>;
}

// Developers can use service to customize blockchain business
//
// It contains:
// - hooks: A pair of hooks that allow inserting a piece of logic before and
//   after the epoch is executed.
// - read: Provide some read-only functions for users or other services to call
// - write: provide some writable functions for users or other services to call
pub trait Service {
    // Executed before the epoch is executed.
    fn hook_before(&mut self) -> ProtocolResult<()> {
        Ok(())
    }

    // Executed after epoch execution.
    fn hook_after(&mut self) -> ProtocolResult<()> {
        Ok(())
    }

    fn write<Context: RequestContext>(&mut self, ctx: Context) -> ProtocolResult<json::JsonValue>;

    fn read<Context: RequestContext>(&self, ctx: Context) -> ProtocolResult<json::JsonValue>;
}

// `ServiceSDK` provides multiple rich interfaces for `service` developers
//
// It contains:
//
// - Various data structures that store data to `world state`(call
//   `alloc_or_recover_*`)
// - Access and modify `account`
// - Event triggered
// - Access to data on the chain (epoch, transaction, receipt)
// - Read / write other `service`
//
// In fact, these functions depend on:
//
// - ChainDB
// - ServiceState
pub trait ServiceSDK {
    // Alloc or recover a `Map` by` var_name`
    fn alloc_or_recover_map<Key: FixedCodec, Value: FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreMap<Key, Value>>>;

    // Alloc or recover a `Array` by` var_name`
    fn alloc_or_recover_array<Elm: FixedCodec>(
        &mut self,
        var_name: &str,
    ) -> ProtocolResult<Box<dyn StoreArray<Elm>>>;

    // Alloc or recover a `Uint64` by` var_name`
    fn alloc_or_recover_uint64(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreUint64>>;

    // Alloc or recover a `String` by` var_name`
    fn alloc_or_recover_string(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreString>>;

    // Alloc or recover a `Bool` by` var_name`
    fn alloc_or_recover_bool(&mut self, var_name: &str) -> ProtocolResult<Box<dyn StoreBool>>;

    // Alloc or recover a `Object` by` var_name`
    fn alloc_or_recover_object<Object: FixedCodec + Default>(
        &mut self,
        var_name: &str,
    ) -> ProtocolResult<Object>;

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

    // Get a epoch by `epoch_id`
    // if not found on the chain, return None
    // When the parameter `epoch_id` is None, get the latest (executing)` epoch`
    fn get_epoch_by_epoch_id(&self, epoch_id: Option<u64>) -> ProtocolResult<Option<Epoch>>;

    // Get a receipt by `tx_hash`
    // if not found on the chain, return None
    fn get_receipt_by_hash(&self, tx_hash: &Hash) -> ProtocolResult<Option<Receipt>>;

    // Trigger an `event`, which can be any string
    // NOTE: The string is recommended as json string
    fn emit_event(&mut self, message: json::JsonValue) -> ProtocolResult<()>;

    // Call other read-only methods of `service` and return the results
    // synchronously NOTE: You can use recursive calls, but the maximum call
    // stack is 1024
    fn read(&self, servide: &str, method: &str, payload: &str) -> ProtocolResult<json::JsonValue>;

    // Call other writable methods of `service` and return the results synchronously
    // NOTE: You can use recursive calls, but the maximum call stack is 1024
    fn write(
        &mut self,
        servide: &str,
        method: &str,
        payload: &str,
    ) -> ProtocolResult<json::JsonValue>;
}

pub trait StoreMap<Key: FixedCodec + PartialEq, Value: FixedCodec> {
    fn get(&self, key: &Key) -> ProtocolResult<Value>;

    fn contains(&self, key: &Key) -> ProtocolResult<bool>;

    fn insert(&mut self, key: Key, value: Value) -> ProtocolResult<()>;

    fn remove(&mut self, key: &Key) -> ProtocolResult<()>;

    fn len(&self) -> ProtocolResult<usize>;

    fn for_each<F>(&mut self, f: F) -> ProtocolResult<()>
    where
        Self: Sized,
        F: FnMut(&mut Value) -> ProtocolResult<()>;
}

pub trait StoreArray<Elm: FixedCodec> {
    fn get(&self, index: usize) -> ProtocolResult<Elm>;

    fn push(&mut self, element: Elm) -> ProtocolResult<()>;

    fn remove(&mut self, index: usize) -> ProtocolResult<()>;

    fn len(&self) -> ProtocolResult<usize>;

    fn for_each<F>(&mut self, f: F) -> ProtocolResult<()>
    where
        Self: Sized,
        F: FnMut(&mut Elm) -> ProtocolResult<()>;
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

    fn len(&self) -> ProtocolResult<usize>;

    fn is_empty(&self) -> ProtocolResult<bool>;
}

pub trait StoreBool {
    fn get(&self) -> ProtocolResult<bool>;

    fn set(&mut self, b: bool) -> ProtocolResult<()>;
}
