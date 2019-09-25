# How to develop a core crate.

> This document will show you how to develop a core crate.

Now, take a look at an example, we are going to develop a `storage` crate, which is used to store data from the blockchain.

## Step 0 Define the trait of the crate.

```rust
// muta/protocol/src/traits/storage.rs

use async_trait::async_trait;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> ProtocolResult<()>;

    async fn get_transaction_by_hash(
        &self,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>>;
}
```

**Starting with the first line of code, you will first see a macro: `#[async_trait]`:**

[`#[async_trait]`](https://crates.io/crates/async-trait) is a macro that allows you to define `async fn` in a `trait`. In most cases you should use async to define your `fn`.

**Next is the second line, a trait declaration:**

Here we constrain this trait to `Send` + `Sync`,if you don't understand the semantics of `Send` and `Sync` you can get knowledge from [the official documentation](https://doc.rust-lang.org/std/marker/index.html).

In short, this constraint is necessary because our runtime is always asynchronous, and you must ensure that your crate satisfies the constraints under asynchronous conditions.

**Define the function signature:**

You only need to pay attention to two points:

1. Always use `&self` and handle the internal variables yourself.
2. The return value is uniformly used with `ProtocolResult<T>`, `ProtocolResult` is wrap to `Result <T, ProtocolError>`, and `ProtocolError` is a global error type.

## Step 1 The adapter that defines crate

Earlier we mentioned that the role of `storage` is to store blockchain data, but it does not care where the final data is stored. It can be memory, network database, hard drive, etc.

The existence of a `StorageAdapter` is decoupled persistence logic that specifies a set of key-value database interfaces that implement various `StorageAdapters` to enforce data persistence requirements in a variety of situations.

```rust
// muta/protocol/src/traits/storage.rs

use async_trait::async_trait;
use bytes::Bytes;

#[async_trait]
pub trait Storage<Adapter: StorageAdapter>: Send + Sync {
    async fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> ProtocolResult<()>;

    async fn get_transaction_by_hash(
        &self,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>>;
}

#[async_trait]
pub trait StorageAdapter: Send + Sync {
    async fn get(&self, c: StorageCategory, key: Bytes) -> ProtocolResult<Option<Bytes>>;

    async fn get_batch(
        &self,
        c: StorageCategory,
        keys: Vec<Bytes>,
    ) -> ProtocolResult<Vec<Option<Bytes>>>;

    async fn insert(&self, c: StorageCategory, key: Bytes, value: Bytes) -> ProtocolResult<()>;

    async fn insert_batch(
        &self,
        c: StorageCategory,
        keys: Vec<Bytes>,
        values: Vec<Bytes>,
    ) -> ProtocolResult<()>;

    async fn contains(&self, c: StorageCategory, key: Bytes) -> ProtocolResult<bool>;

    async fn remove(&self, c: StorageCategory, key: Bytes) -> ProtocolResult<()>;

    async fn remove_batch(&self, c: StorageCategory, keys: Vec<Bytes>) -> ProtocolResult<()>;
}
```

Finally, don't forget to add the `pub trait Storage<Adapter: StorageAdapter>` constraint to the `Storage`. Its purpose is to make you remember to always rely on an adapter.

## Step 3 Implement storage crate

See: https://github.com/nervosnetwork/muta/blob/master/core/storage/src/lib.rs

Note:

1. Core crate does not allow dependencies on other cores crate.
2. The adapter can rely on other core crate.
