# 存储设计

## 目标

存储模块负责对上层模块提供数据的的持久化支持，采用 key-value 数据库。

模块主要分两个组件：

- Storage
- Adapter

Storage 组件为上层组件提供了统一的数据存储接口，而 Adapter 则负责对更底层
的数据库具体实现提供统一的抽象接口，使得不同的数据库实现可以方便的接入。

## 设计

Storage 接口设计主要和业务逻辑有关，大体分为：

- Transaction 交易数据存储
- Receipt 交易回执存储
- Epoch 块存储
- LatestProof 当前最新共识的 Proof，同步需要

Adapter 接口负责上述 Storage 逻辑对应的数据结构，通过 Protocol 提供的
Codec 序列化接口，完成对数据的存储操作。大体操作可以分为：

- get
- insert
- remove
- iter
- batch_modify

## 接口

### Storage

#### 基本上通用 CR

```rust
async fn get_xxx(&self, key: XXX) -> ProtocolResult<XXX>;
async fn insert_xxx(&self, key: XXX, value: XXX) -> ProtocolResult<()>;
async fn contains_xxx(&self, key: XXX) -> ProtocolResult<bool>;
```

没有更新和删除接口，以下是例外

LatestProof 有更新接口，且是固定的 Key。

### Adapter

```rust
#[derive(Debug, Copy, Clone, Display)]
pub enum StorageCategory {
    Epoch,
    Receipt,
    SignedTransaction,
}

pub trait StorageSchema {
    type Key: ProtocolCodec + Send;
    type Value: ProtocolCodec + Send;

    fn category() -> StorageCategory;
}

#[async_trait]
pub trait Storage<Adapter: StorageAdapter>: Send + Sync {
    async fn insert_transactions(&self, signed_txs: Vec<SignedTransaction>) -> ProtocolResult<()>;

    async fn get_transaction_by_hash(
        &self,
        tx_hash: Hash,
    ) -> ProtocolResult<Option<SignedTransaction>>;
}

pub enum StorageBatchModify<S: StorageSchema> {
    Remove,
    Insert(<S as StorageSchema>::Value),
}

#[async_trait]
pub trait StorageAdapter: Sync + Send {
    async fn insert<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
        val: <S as StorageSchema>::Value,
    ) -> ProtocolResult<()>;

    async fn get<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<Option<<S as StorageSchema>::Value>>;

    async fn remove<S: StorageSchema>(&self, key: <S as StorageSchema>::Key) -> ProtocolResult<()>;

    async fn contains<S: StorageSchema>(
        &self,
        key: <S as StorageSchema>::Key,
    ) -> ProtocolResult<bool>;

    // TODO: Query struct?
    fn iter<S: StorageSchema + 'static>(
        &self,
        keys: Vec<<S as StorageSchema>::Key>,
    ) -> Box<dyn Stream<Item = ProtocolResult<Option<<S as StorageSchema>::Value>>>>;

    async fn batch_modify<S: StorageSchema>(
        &self,
        keys: Vec<<S as StorageSchema>::Key>,
        vals: Vec<StorageBatchModify<S>>,
    ) -> ProtocolResult<()>;
}
```

Adapter 通过 Schema 和 Protocol Codec， 直接对应表的数据结构进行序列化和反序列化操作。Storage 层无需关心序列化和反序列操作，拿到的直接就是对应的数据结构。

使用 Stream 实现异步原生的遍历，批量读取操作。

BatchModify 则将插入和删除进行了整合，接口稍简洁干净一些。
