#![feature(prelude_import)]
#![feature(test)]
#[prelude_import]
use std::prelude::v1::*;
#[macro_use]
extern crate std;
pub mod adapter {
    pub mod memory {
        use std::collections::HashMap;
        use std::error::Error;
        use std::sync::Arc;
        use async_trait::async_trait;
        use derive_more::{Display, From};
        use parking_lot::RwLock;
        use protocol::codec::ProtocolCodec;
        use protocol::traits::{StorageAdapter, StorageBatchModify, StorageSchema};
        use protocol::Bytes;
        use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};
        pub struct MemoryAdapter {
            db: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
        }
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for MemoryAdapter {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match *self {
                    MemoryAdapter { db: ref __self_0_0 } => {
                        let mut debug_trait_builder = f.debug_struct("MemoryAdapter");
                        let _ = debug_trait_builder.field("db", &&(*__self_0_0));
                        debug_trait_builder.finish()
                    }
                }
            }
        }
        impl MemoryAdapter {
            pub fn new() -> Self {
                MemoryAdapter {
                    db: Arc::new(RwLock::new(HashMap::new())),
                }
            }
        }
        impl Default for MemoryAdapter {
            fn default() -> Self {
                MemoryAdapter {
                    db: Arc::new(RwLock::new(HashMap::new())),
                }
            }
        }
        impl StorageAdapter for MemoryAdapter {
            fn insert<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                key: <S as StorageSchema>::Key,
                val: <S as StorageSchema>::Value,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<Output = ProtocolResult<()>>
                        + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __insert<S: StorageSchema>(
                    _self: &MemoryAdapter,
                    mut key: <S as StorageSchema>::Key,
                    mut val: <S as StorageSchema>::Value,
                ) -> ProtocolResult<()> {
                    let key = key.encode().await?.to_vec();
                    let val = val.encode().await?.to_vec();
                    _self.db.write().insert(key, val);
                    Ok(())
                }
                Box::pin(__insert::<S>(self, key, val))
            }
            fn get<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                key: <S as StorageSchema>::Key,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<
                            Output = ProtocolResult<Option<<S as StorageSchema>::Value>>,
                        > + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get<S: StorageSchema>(
                    _self: &MemoryAdapter,
                    mut key: <S as StorageSchema>::Key,
                ) -> ProtocolResult<Option<<S as StorageSchema>::Value>> {
                    let key = key.encode().await?;
                    let opt_bytes = _self.db.read().get(&key.to_vec()).cloned();
                    if let Some(bytes) = opt_bytes {
                        let val = <_>::decode(bytes).await?;
                        Ok(Some(val))
                    } else {
                        Ok(None)
                    }
                }
                Box::pin(__get::<S>(self, key))
            }
            fn remove<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                key: <S as StorageSchema>::Key,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<Output = ProtocolResult<()>>
                        + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __remove<S: StorageSchema>(
                    _self: &MemoryAdapter,
                    mut key: <S as StorageSchema>::Key,
                ) -> ProtocolResult<()> {
                    let key = key.encode().await?.to_vec();
                    _self.db.write().remove(&key);
                    Ok(())
                }
                Box::pin(__remove::<S>(self, key))
            }
            fn contains<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                key: <S as StorageSchema>::Key,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<Output = ProtocolResult<bool>>
                        + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __contains<S: StorageSchema>(
                    _self: &MemoryAdapter,
                    mut key: <S as StorageSchema>::Key,
                ) -> ProtocolResult<bool> {
                    let key = key.encode().await?.to_vec();
                    Ok(_self.db.read().get(&key).is_some())
                }
                Box::pin(__contains::<S>(self, key))
            }
            fn batch_modify<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                keys: Vec<<S as StorageSchema>::Key>,
                vals: Vec<StorageBatchModify<S>>,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<Output = ProtocolResult<()>>
                        + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __batch_modify<S: StorageSchema>(
                    _self: &MemoryAdapter,
                    keys: Vec<<S as StorageSchema>::Key>,
                    vals: Vec<StorageBatchModify<S>>,
                ) -> ProtocolResult<()> {
                    if keys.len() != vals.len() {
                        return Err(MemoryAdapterError::BatchLengthMismatch.into());
                    }
                    let mut pairs: Vec<(Bytes, Option<Bytes>)> = Vec::with_capacity(keys.len());
                    for (mut key, value) in keys.into_iter().zip(vals.into_iter()) {
                        let key = key.encode().await?;
                        let value = match value {
                            StorageBatchModify::Insert(mut value) => Some(value.encode().await?),
                            StorageBatchModify::Remove => None,
                        };
                        pairs.push((key, value))
                    }
                    for (key, value) in pairs.into_iter() {
                        match value {
                            Some(value) => _self.db.write().insert(key.to_vec(), value.to_vec()),
                            None => _self.db.write().remove(&key.to_vec()),
                        };
                    }
                    Ok(())
                }
                Box::pin(__batch_modify::<S>(self, keys, vals))
            }
        }
        pub enum MemoryAdapterError {
            #[display(fmt = "batch length dont match")]
            BatchLengthMismatch,
        }
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for MemoryAdapterError {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match (&*self,) {
                    (&MemoryAdapterError::BatchLengthMismatch,) => {
                        let mut debug_trait_builder = f.debug_tuple("BatchLengthMismatch");
                        debug_trait_builder.finish()
                    }
                }
            }
        }
        impl ::std::fmt::Display for MemoryAdapterError {
            #[allow(unused_variables)]
            #[inline]
            fn fmt(
                &self,
                _derive_more_Display_formatter: &mut ::std::fmt::Formatter,
            ) -> ::std::fmt::Result {
                match self {
                    MemoryAdapterError::BatchLengthMismatch => _derive_more_Display_formatter
                        .write_fmt(::core::fmt::Arguments::new_v1(
                            &["batch length dont match"],
                            &match () {
                                () => [],
                            },
                        )),
                    _ => Ok(()),
                }
            }
        }
        impl ::std::convert::From<()> for MemoryAdapterError {
            #[allow(unused_variables)]
            #[inline]
            fn from(original: ()) -> MemoryAdapterError {
                MemoryAdapterError::BatchLengthMismatch {}
            }
        }
        impl Error for MemoryAdapterError {}
        impl From<MemoryAdapterError> for ProtocolError {
            fn from(err: MemoryAdapterError) -> ProtocolError {
                ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
            }
        }
    }
    pub mod rocks {
        use std::error::Error;
        use std::path::Path;
        use std::sync::Arc;
        use async_trait::async_trait;
        use derive_more::{Display, From};
        use rocksdb::{ColumnFamily, Options, WriteBatch, DB};
        use protocol::codec::ProtocolCodec;
        use protocol::traits::{StorageAdapter, StorageBatchModify, StorageCategory, StorageSchema};
        use protocol::Bytes;
        use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};
        pub struct RocksAdapter {
            db: Arc<DB>,
        }
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for RocksAdapter {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match *self {
                    RocksAdapter { db: ref __self_0_0 } => {
                        let mut debug_trait_builder = f.debug_struct("RocksAdapter");
                        let _ = debug_trait_builder.field("db", &&(*__self_0_0));
                        debug_trait_builder.finish()
                    }
                }
            }
        }
        impl RocksAdapter {
            pub fn new<P: AsRef<Path>>(path: P, max_open_files: i32) -> ProtocolResult<Self> {
                let mut opts = Options::default();
                opts.create_if_missing(true);
                opts.create_missing_column_families(true);
                opts.set_max_open_files(max_open_files);
                let categories = [
                    map_category(StorageCategory::Block),
                    map_category(StorageCategory::Receipt),
                    map_category(StorageCategory::SignedTransaction),
                    map_category(StorageCategory::Wal),
                ];
                let db =
                    DB::open_cf(&opts, path, categories.iter()).map_err(RocksAdapterError::from)?;
                Ok(RocksAdapter { db: Arc::new(db) })
            }
        }
        impl StorageAdapter for RocksAdapter {
            fn insert<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                key: <S as StorageSchema>::Key,
                val: <S as StorageSchema>::Value,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<Output = ProtocolResult<()>>
                        + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __insert<S: StorageSchema>(
                    _self: &RocksAdapter,
                    mut key: <S as StorageSchema>::Key,
                    mut val: <S as StorageSchema>::Value,
                ) -> ProtocolResult<()> {
                    let column = get_column::<S>(&_self.db)?;
                    let key = key.encode().await?.to_vec();
                    let val = val.encode().await?.to_vec();
                    _self
                        .db
                        .put_cf(column, key, val)
                        .map_err(RocksAdapterError::from)?;
                    Ok(())
                }
                Box::pin(__insert::<S>(self, key, val))
            }
            fn get<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                key: <S as StorageSchema>::Key,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<
                            Output = ProtocolResult<Option<<S as StorageSchema>::Value>>,
                        > + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get<S: StorageSchema>(
                    _self: &RocksAdapter,
                    mut key: <S as StorageSchema>::Key,
                ) -> ProtocolResult<Option<<S as StorageSchema>::Value>> {
                    let column = get_column::<S>(&_self.db)?;
                    let key = key.encode().await?;
                    let opt_bytes = {
                        _self
                            .db
                            .get_cf(column, key)
                            .map_err(RocksAdapterError::from)?
                            .map(|db_vec| Bytes::from(db_vec.to_vec()))
                    };
                    if let Some(bytes) = opt_bytes {
                        let val = <_>::decode(bytes).await?;
                        Ok(Some(val))
                    } else {
                        Ok(None)
                    }
                }
                Box::pin(__get::<S>(self, key))
            }
            fn remove<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                key: <S as StorageSchema>::Key,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<Output = ProtocolResult<()>>
                        + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __remove<S: StorageSchema>(
                    _self: &RocksAdapter,
                    mut key: <S as StorageSchema>::Key,
                ) -> ProtocolResult<()> {
                    let column = get_column::<S>(&_self.db)?;
                    let key = key.encode().await?.to_vec();
                    _self
                        .db
                        .delete_cf(column, key)
                        .map_err(RocksAdapterError::from)?;
                    Ok(())
                }
                Box::pin(__remove::<S>(self, key))
            }
            fn contains<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                key: <S as StorageSchema>::Key,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<Output = ProtocolResult<bool>>
                        + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __contains<S: StorageSchema>(
                    _self: &RocksAdapter,
                    mut key: <S as StorageSchema>::Key,
                ) -> ProtocolResult<bool> {
                    let column = get_column::<S>(&_self.db)?;
                    let key = key.encode().await?.to_vec();
                    let val = _self
                        .db
                        .get_cf(column, key)
                        .map_err(RocksAdapterError::from)?;
                    Ok(val.is_some())
                }
                Box::pin(__contains::<S>(self, key))
            }
            fn batch_modify<'life0, 'async_trait, S: StorageSchema>(
                &'life0 self,
                keys: Vec<<S as StorageSchema>::Key>,
                vals: Vec<StorageBatchModify<S>>,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<Output = ProtocolResult<()>>
                        + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                S: 'async_trait,
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __batch_modify<S: StorageSchema>(
                    _self: &RocksAdapter,
                    keys: Vec<<S as StorageSchema>::Key>,
                    vals: Vec<StorageBatchModify<S>>,
                ) -> ProtocolResult<()> {
                    if keys.len() != vals.len() {
                        return Err(RocksAdapterError::BatchLengthMismatch.into());
                    }
                    let column = get_column::<S>(&_self.db)?;
                    let mut pairs: Vec<(Bytes, Option<Bytes>)> = Vec::with_capacity(keys.len());
                    for (mut key, value) in keys.into_iter().zip(vals.into_iter()) {
                        let key = key.encode().await?;
                        let value = match value {
                            StorageBatchModify::Insert(mut value) => Some(value.encode().await?),
                            StorageBatchModify::Remove => None,
                        };
                        pairs.push((key, value))
                    }
                    let mut batch = WriteBatch::default();
                    for (key, value) in pairs.into_iter() {
                        match value {
                            Some(value) => batch
                                .put_cf(column, key, value)
                                .map_err(RocksAdapterError::from)?,
                            None => batch
                                .delete_cf(column, key)
                                .map_err(RocksAdapterError::from)?,
                        }
                    }
                    _self.db.write(batch).map_err(RocksAdapterError::from)?;
                    Ok(())
                }
                Box::pin(__batch_modify::<S>(self, keys, vals))
            }
        }
        pub enum RocksAdapterError {
            #[display(fmt = "category {} not found", _0)]
            CategoryNotFound(&'static str),
            #[display(fmt = "rocksdb {}", _0)]
            RocksDB(rocksdb::Error),
            #[display(fmt = "parameters do not match")]
            InsertParameter,
            #[display(fmt = "batch length dont match")]
            BatchLengthMismatch,
        }
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for RocksAdapterError {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match (&*self,) {
                    (&RocksAdapterError::CategoryNotFound(ref __self_0),) => {
                        let mut debug_trait_builder = f.debug_tuple("CategoryNotFound");
                        let _ = debug_trait_builder.field(&&(*__self_0));
                        debug_trait_builder.finish()
                    }
                    (&RocksAdapterError::RocksDB(ref __self_0),) => {
                        let mut debug_trait_builder = f.debug_tuple("RocksDB");
                        let _ = debug_trait_builder.field(&&(*__self_0));
                        debug_trait_builder.finish()
                    }
                    (&RocksAdapterError::InsertParameter,) => {
                        let mut debug_trait_builder = f.debug_tuple("InsertParameter");
                        debug_trait_builder.finish()
                    }
                    (&RocksAdapterError::BatchLengthMismatch,) => {
                        let mut debug_trait_builder = f.debug_tuple("BatchLengthMismatch");
                        debug_trait_builder.finish()
                    }
                }
            }
        }
        impl ::std::fmt::Display for RocksAdapterError {
            #[allow(unused_variables)]
            #[inline]
            fn fmt(
                &self,
                _derive_more_Display_formatter: &mut ::std::fmt::Formatter,
            ) -> ::std::fmt::Result {
                match self {
                    RocksAdapterError::CategoryNotFound(_0) => _derive_more_Display_formatter
                        .write_fmt(::core::fmt::Arguments::new_v1(
                            &["category ", " not found"],
                            &match (&_0,) {
                                (arg0,) => [::core::fmt::ArgumentV1::new(
                                    arg0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        )),
                    RocksAdapterError::RocksDB(_0) => {
                        _derive_more_Display_formatter.write_fmt(::core::fmt::Arguments::new_v1(
                            &["rocksdb "],
                            &match (&_0,) {
                                (arg0,) => [::core::fmt::ArgumentV1::new(
                                    arg0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        ))
                    }
                    RocksAdapterError::InsertParameter => {
                        _derive_more_Display_formatter.write_fmt(::core::fmt::Arguments::new_v1(
                            &["parameters do not match"],
                            &match () {
                                () => [],
                            },
                        ))
                    }
                    RocksAdapterError::BatchLengthMismatch => _derive_more_Display_formatter
                        .write_fmt(::core::fmt::Arguments::new_v1(
                            &["batch length dont match"],
                            &match () {
                                () => [],
                            },
                        )),
                    _ => Ok(()),
                }
            }
        }
        impl ::std::convert::From<(&'static str)> for RocksAdapterError {
            #[allow(unused_variables)]
            #[inline]
            fn from(original: (&'static str)) -> RocksAdapterError {
                RocksAdapterError::CategoryNotFound(original)
            }
        }
        impl ::std::convert::From<(rocksdb::Error)> for RocksAdapterError {
            #[allow(unused_variables)]
            #[inline]
            fn from(original: (rocksdb::Error)) -> RocksAdapterError {
                RocksAdapterError::RocksDB(original)
            }
        }
        impl Error for RocksAdapterError {}
        impl From<RocksAdapterError> for ProtocolError {
            fn from(err: RocksAdapterError) -> ProtocolError {
                ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
            }
        }
        const C_BLOCKS: &str = "c1";
        const C_SIGNED_TRANSACTIONS: &str = "c2";
        const C_RECEIPTS: &str = "c3";
        const C_WALS: &str = "c4";
        fn map_category(c: StorageCategory) -> &'static str {
            match c {
                StorageCategory::Block => C_BLOCKS,
                StorageCategory::Receipt => C_RECEIPTS,
                StorageCategory::SignedTransaction => C_SIGNED_TRANSACTIONS,
                StorageCategory::Wal => C_WALS,
            }
        }
        fn get_column<S: StorageSchema>(db: &DB) -> Result<ColumnFamily, RocksAdapterError> {
            let category = map_category(S::category());
            let column = db
                .cf_handle(category)
                .ok_or_else(|| RocksAdapterError::from(category))?;
            Ok(column)
        }
    }
}
use std::convert::From;
use std::error::Error;
use std::sync::Arc;
use async_trait::async_trait;
use derive_more::{Display, From};
use lazy_static::lazy_static;
use tokio::sync::RwLock;
use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    Context, Storage, StorageAdapter, StorageBatchModify, StorageCategory, StorageSchema,
};
use protocol::types::{Block, Hash, Proof, Receipt, SignedTransaction};
use protocol::Bytes;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};
#[allow(missing_copy_implementations)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub struct LATEST_BLOCK_KEY {
    __private_field: (),
}
#[doc(hidden)]
pub static LATEST_BLOCK_KEY: LATEST_BLOCK_KEY = LATEST_BLOCK_KEY {
    __private_field: (),
};
impl ::lazy_static::__Deref for LATEST_BLOCK_KEY {
    type Target = Hash;
    fn deref(&self) -> &Hash {
        #[inline(always)]
        fn __static_ref_initialize() -> Hash {
            Hash::digest(Bytes::from("latest_hash"))
        }
        #[inline(always)]
        fn __stability() -> &'static Hash {
            static LAZY: ::lazy_static::lazy::Lazy<Hash> = ::lazy_static::lazy::Lazy::INIT;
            LAZY.get(__static_ref_initialize)
        }
        __stability()
    }
}
impl ::lazy_static::LazyStatic for LATEST_BLOCK_KEY {
    fn initialize(lazy: &Self) {
        let _ = &**lazy;
    }
}
#[allow(missing_copy_implementations)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub struct LATEST_PROOF_KEY {
    __private_field: (),
}
#[doc(hidden)]
pub static LATEST_PROOF_KEY: LATEST_PROOF_KEY = LATEST_PROOF_KEY {
    __private_field: (),
};
impl ::lazy_static::__Deref for LATEST_PROOF_KEY {
    type Target = Hash;
    fn deref(&self) -> &Hash {
        #[inline(always)]
        fn __static_ref_initialize() -> Hash {
            Hash::digest(Bytes::from("latest_proof"))
        }
        #[inline(always)]
        fn __stability() -> &'static Hash {
            static LAZY: ::lazy_static::lazy::Lazy<Hash> = ::lazy_static::lazy::Lazy::INIT;
            LAZY.get(__static_ref_initialize)
        }
        __stability()
    }
}
impl ::lazy_static::LazyStatic for LATEST_PROOF_KEY {
    fn initialize(lazy: &Self) {
        let _ = &**lazy;
    }
}
#[allow(missing_copy_implementations)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub struct OVERLORD_WAL_KEY {
    __private_field: (),
}
#[doc(hidden)]
pub static OVERLORD_WAL_KEY: OVERLORD_WAL_KEY = OVERLORD_WAL_KEY {
    __private_field: (),
};
impl ::lazy_static::__Deref for OVERLORD_WAL_KEY {
    type Target = Hash;
    fn deref(&self) -> &Hash {
        #[inline(always)]
        fn __static_ref_initialize() -> Hash {
            Hash::digest(Bytes::from("overlord_wal"))
        }
        #[inline(always)]
        fn __stability() -> &'static Hash {
            static LAZY: ::lazy_static::lazy::Lazy<Hash> = ::lazy_static::lazy::Lazy::INIT;
            LAZY.get(__static_ref_initialize)
        }
        __stability()
    }
}
impl ::lazy_static::LazyStatic for OVERLORD_WAL_KEY {
    fn initialize(lazy: &Self) {
        let _ = &**lazy;
    }
}
pub struct ImplStorage<Adapter> {
    adapter: Arc<Adapter>,
    latest_block: RwLock<Option<Block>>,
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<Adapter: ::core::fmt::Debug> ::core::fmt::Debug for ImplStorage<Adapter> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        match *self {
            ImplStorage {
                adapter: ref __self_0_0,
                latest_block: ref __self_0_1,
            } => {
                let mut debug_trait_builder = f.debug_struct("ImplStorage");
                let _ = debug_trait_builder.field("adapter", &&(*__self_0_0));
                let _ = debug_trait_builder.field("latest_block", &&(*__self_0_1));
                debug_trait_builder.finish()
            }
        }
    }
}
impl<Adapter: StorageAdapter> ImplStorage<Adapter> {
    pub fn new(adapter: Arc<Adapter>) -> Self {
        Self {
            adapter,
            latest_block: RwLock::new(None),
        }
    }
}
pub struct TransactionSchema;
impl StorageSchema for TransactionSchema {
    type Key = Hash;
    type Value = SignedTransaction;
    fn category() -> StorageCategory {
        StorageCategory::SignedTransaction
    }
}
pub struct ReceiptSchema;
impl StorageSchema for ReceiptSchema {
    type Key = Hash;
    type Value = Receipt;
    fn category() -> StorageCategory {
        StorageCategory::Receipt
    }
}
pub struct BlockSchema;
impl StorageSchema for BlockSchema {
    type Key = u64;
    type Value = Block;
    fn category() -> StorageCategory {
        StorageCategory::Block
    }
}
pub struct HashBlockSchema;
impl StorageSchema for HashBlockSchema {
    type Key = Hash;
    type Value = u64;
    fn category() -> StorageCategory {
        StorageCategory::Block
    }
}
pub struct LatestBlockSchema;
impl StorageSchema for LatestBlockSchema {
    type Key = Hash;
    type Value = Block;
    fn category() -> StorageCategory {
        StorageCategory::Block
    }
}
pub struct LatestProofSchema;
impl StorageSchema for LatestProofSchema {
    type Key = Hash;
    type Value = Proof;
    fn category() -> StorageCategory {
        StorageCategory::Block
    }
}
pub struct OverlordWalSchema;
impl StorageSchema for OverlordWalSchema {
    type Key = Hash;
    type Value = Bytes;
    fn category() -> StorageCategory {
        StorageCategory::Wal
    }
}
impl<Adapter: StorageAdapter> Storage for ImplStorage<Adapter> {
    #[allow(unused_variables)]
    fn insert_transactions<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        signed_txs: Vec<SignedTransaction>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<()>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.insert_transactions",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.insert_transactions", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.insert_transactions", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __insert_transactions<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    signed_txs: Vec<SignedTransaction>,
                ) -> ProtocolResult<()> {
                    let mut hashes = Vec::with_capacity(signed_txs.len());
                    for item in signed_txs.iter() {
                        hashes.push(item.tx_hash.clone())
                    }
                    let batch_insert = signed_txs
                        .into_iter()
                        .map(StorageBatchModify::Insert)
                        .collect::<Vec<_>>();
                    _self
                        .adapter
                        .batch_modify::<TransactionSchema>(hashes, batch_insert)
                        .await?;
                    Ok(())
                }
                Box::pin(__insert_transactions::<Adapter>(self, ctx, signed_txs))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn insert_block<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        block: Block,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<()>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.insert_block",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.insert_block", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.insert_block", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __insert_block<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    block: Block,
                ) -> ProtocolResult<()> {
                    let height = block.header.height;
                    let block_hash = Hash::digest(block.encode_fixed()?);
                    _self
                        .adapter
                        .insert::<BlockSchema>(height.clone(), block.clone())
                        .await?;
                    _self
                        .adapter
                        .insert::<HashBlockSchema>(block_hash, height)
                        .await?;
                    _self
                        .adapter
                        .insert::<LatestBlockSchema>(LATEST_BLOCK_KEY.clone(), block.clone())
                        .await?;
                    _self.latest_block.write().await.replace(block);
                    Ok(())
                }
                Box::pin(__insert_block::<Adapter>(self, ctx, block))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn insert_receipts<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        receipts: Vec<Receipt>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<()>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.insert_receipts",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.insert_receipts", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.insert_receipts", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __insert_receipts<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    receipts: Vec<Receipt>,
                ) -> ProtocolResult<()> {
                    let mut hashes = Vec::with_capacity(receipts.len());
                    for item in receipts.iter() {
                        hashes.push(item.tx_hash.clone())
                    }
                    let batch_insert = receipts
                        .into_iter()
                        .map(StorageBatchModify::Insert)
                        .collect::<Vec<_>>();
                    _self
                        .adapter
                        .batch_modify::<ReceiptSchema>(hashes, batch_insert)
                        .await?;
                    Ok(())
                }
                Box::pin(__insert_receipts::<Adapter>(self, ctx, receipts))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn update_latest_proof<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        proof: Proof,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<()>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.update_latest_proof",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.update_latest_proof", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.update_latest_proof", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __update_latest_proof<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    proof: Proof,
                ) -> ProtocolResult<()> {
                    _self
                        .adapter
                        .insert::<LatestProofSchema>(LATEST_PROOF_KEY.clone(), proof)
                        .await?;
                    Ok(())
                }
                Box::pin(__update_latest_proof::<Adapter>(self, ctx, proof))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn get_transaction_by_hash<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        tx_hash: Hash,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<SignedTransaction>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.get_transaction_by_hash",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.get_transaction_by_hash", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.get_transaction_by_hash", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get_transaction_by_hash<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    tx_hash: Hash,
                ) -> ProtocolResult<SignedTransaction> {
                    let stx = {
                        let opt = _self.adapter.get::<TransactionSchema>(tx_hash).await?;
                        check_none(opt)?
                    };
                    Ok(stx)
                }
                Box::pin(__get_transaction_by_hash::<Adapter>(self, ctx, tx_hash))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn get_transactions<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        hashes: Vec<Hash>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<Vec<SignedTransaction>>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.get_transactions",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.get_transactions", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.get_transactions", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get_transactions<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    hashes: Vec<Hash>,
                ) -> ProtocolResult<Vec<SignedTransaction>> {
                    let stxs = {
                        let opt = _self.adapter.get_batch::<TransactionSchema>(hashes).await?;
                        opts_to_flat(opt)
                    };
                    Ok(stxs)
                }
                Box::pin(__get_transactions::<Adapter>(self, ctx, hashes))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn get_latest_block<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<Block>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.get_latest_block",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.get_latest_block", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.get_latest_block", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get_latest_block<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                ) -> ProtocolResult<Block> {
                    let opt_block = { _self.latest_block.read().await.clone() };
                    if let Some(block) = opt_block {
                        Ok(block)
                    } else {
                        Ok({
                            let opt = _self
                                .adapter
                                .get::<LatestBlockSchema>(LATEST_BLOCK_KEY.clone())
                                .await?;
                            check_none(opt)?
                        })
                    }
                }
                Box::pin(__get_latest_block::<Adapter>(self, ctx))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn get_block_by_height<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        height: u64,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<Block>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.get_block_by_height",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.get_block_by_height", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.get_block_by_height", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get_block_by_height<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    height: u64,
                ) -> ProtocolResult<Block> {
                    let block = {
                        let opt = _self.adapter.get::<BlockSchema>(height).await?;
                        check_none(opt)?
                    };
                    Ok(block)
                }
                Box::pin(__get_block_by_height::<Adapter>(self, ctx, height))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn get_block_by_hash<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        block_hash: Hash,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<Block>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.get_block_by_hash",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.get_block_by_hash", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.get_block_by_hash", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get_block_by_hash<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    block_hash: Hash,
                ) -> ProtocolResult<Block> {
                    let height = {
                        let opt = _self.adapter.get::<HashBlockSchema>(block_hash).await?;
                        check_none(opt)?
                    };
                    let block = {
                        let opt = _self.adapter.get::<BlockSchema>(height).await?;
                        check_none(opt)?
                    };
                    Ok(block)
                }
                Box::pin(__get_block_by_hash::<Adapter>(self, ctx, block_hash))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn get_receipt<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        hash: Hash,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<Receipt>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.get_receipt",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.get_receipt", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.get_receipt", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get_receipt<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    hash: Hash,
                ) -> ProtocolResult<Receipt> {
                    let receipt = {
                        let opt = _self.adapter.get::<ReceiptSchema>(hash).await?;
                        check_none(opt)?
                    };
                    Ok(receipt)
                }
                Box::pin(__get_receipt::<Adapter>(self, ctx, hash))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn get_receipts<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        hashes: Vec<Hash>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<Vec<Receipt>>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.get_receipts",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.get_receipts", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.get_receipts", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get_receipts<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    hashes: Vec<Hash>,
                ) -> ProtocolResult<Vec<Receipt>> {
                    let receipts = {
                        let opt = _self.adapter.get_batch::<ReceiptSchema>(hashes).await?;
                        opts_to_flat(opt)
                    };
                    Ok(receipts)
                }
                Box::pin(__get_receipts::<Adapter>(self, ctx, hashes))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn get_latest_proof<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<Proof>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.get_latest_proof",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.get_latest_proof", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.get_latest_proof", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __get_latest_proof<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                ) -> ProtocolResult<Proof> {
                    let proof = {
                        let opt = _self
                            .adapter
                            .get::<LatestProofSchema>(LATEST_PROOF_KEY.clone())
                            .await?;
                        check_none(opt)?
                    };
                    Ok(proof)
                }
                Box::pin(__get_latest_proof::<Adapter>(self, ctx))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn update_overlord_wal<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
        info: Bytes,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<()>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.update_overlord_wal",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.update_overlord_wal", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.update_overlord_wal", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __update_overlord_wal<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                    info: Bytes,
                ) -> ProtocolResult<()> {
                    _self
                        .adapter
                        .insert::<OverlordWalSchema>(OVERLORD_WAL_KEY.clone(), info)
                        .await?;
                    Ok(())
                }
                Box::pin(__update_overlord_wal::<Adapter>(self, ctx, info))
            }
            .await
        })
    }
    #[allow(unused_variables)]
    fn load_overlord_wal<'life0, 'async_trait>(
        &'life0 self,
        ctx: Context,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ProtocolResult<Bytes>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        use muta_apm::rustracing_jaeger::span::SpanContext;
        use muta_apm::rustracing::tag::Tag;
        use muta_apm::rustracing::log::LogField;
        let mut span_tags: Vec<Tag> = Vec::new();
        span_tags.push(Tag::new("kind", "storage"));
        let mut span_logs: Vec<LogField> = Vec::new();
        let mut span = if let Some(parent_ctx) = ctx.get::<Option<SpanContext>>("parent_span_ctx") {
            if parent_ctx.is_some() {
                muta_apm::MUTA_TRACER.child_of_span(
                    "storage.load_overlord_wal",
                    parent_ctx.clone().unwrap(),
                    span_tags,
                )
            } else {
                muta_apm::MUTA_TRACER.span("storage.load_overlord_wal", span_tags)
            }
        } else {
            muta_apm::MUTA_TRACER.span("storage.load_overlord_wal", span_tags)
        };
        let ctx = match span.as_mut() {
            Some(span) => {
                span.log(|log| {
                    for span_log in span_logs.into_iter() {
                        log.field(span_log);
                    }
                });
                ctx.with_value(
                    "parent_span_ctx",
                    span.context().map(span.context().cloned()),
                )
            }
            None => ctx,
        };
        Box::pin(async move {
            let _ = span;
            {
                #[allow(clippy::missing_docs_in_private_items, clippy::used_underscore_binding)]
                async fn __load_overlord_wal<Adapter: StorageAdapter>(
                    _self: &ImplStorage<Adapter>,
                    ctx: Context,
                ) -> ProtocolResult<Bytes> {
                    let wal_info = {
                        let opt = _self
                            .adapter
                            .get::<OverlordWalSchema>(OVERLORD_WAL_KEY.clone())
                            .await?;
                        check_none(opt)?
                    };
                    Ok(wal_info)
                }
                Box::pin(__load_overlord_wal::<Adapter>(self, ctx))
            }
            .await
        })
    }
}
fn opts_to_flat<T>(values: Vec<Option<T>>) -> Vec<T> {
    values
        .into_iter()
        .filter(Option::is_some)
        .map(|v| v.expect("get value"))
        .collect()
}
fn check_none<T>(opt: Option<T>) -> ProtocolResult<T> {
    opt.ok_or_else(|| StorageError::GetNone.into())
}
pub enum StorageError {
    #[display(fmt = "get none")]
    GetNone,
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::fmt::Debug for StorageError {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        match (&*self,) {
            (&StorageError::GetNone,) => {
                let mut debug_trait_builder = f.debug_tuple("GetNone");
                debug_trait_builder.finish()
            }
        }
    }
}
impl ::std::fmt::Display for StorageError {
    #[allow(unused_variables)]
    #[inline]
    fn fmt(
        &self,
        _derive_more_Display_formatter: &mut ::std::fmt::Formatter,
    ) -> ::std::fmt::Result {
        match self {
            StorageError::GetNone => {
                _derive_more_Display_formatter.write_fmt(::core::fmt::Arguments::new_v1(
                    &["get none"],
                    &match () {
                        () => [],
                    },
                ))
            }
            _ => Ok(()),
        }
    }
}
impl ::std::convert::From<()> for StorageError {
    #[allow(unused_variables)]
    #[inline]
    fn from(original: ()) -> StorageError {
        StorageError::GetNone {}
    }
}
impl Error for StorageError {}
impl From<StorageError> for ProtocolError {
    fn from(err: StorageError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Storage, Box::new(err))
    }
}
