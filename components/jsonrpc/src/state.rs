//! A middleware for JSONRPC and Muta blockchain.
#![allow(clippy::needless_lifetimes)]

use futures::compat::Future01CompatExt;
use std::sync::Arc;

use core_context::Context;
use core_runtime::{Executor, TransactionPool};
use core_serialization::AsyncCodec;
use core_storage::Storage;
use core_types::{Address, Block, Hash};
use log;
use numext_fixed_uint::U256;

use crate::error::RpcError;
use crate::types::cita;
use crate::types::TxResponse;
use crate::util;
use crate::RpcResult;

pub struct AppState<E, T, S> {
    executor: Arc<E>,
    transaction_pool: Arc<T>,
    storage: Arc<S>,
}

impl<E, T, S> Clone for AppState<E, T, S> {
    fn clone(&self) -> Self {
        Self {
            executor: Arc::<E>::clone(&self.executor),
            transaction_pool: Arc::<T>::clone(&self.transaction_pool),
            storage: Arc::<S>::clone(&self.storage),
        }
    }
}

impl<E, T, S> AppState<E, T, S>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
{
    pub fn new(executor: Arc<E>, transaction_pool: Arc<T>, storage: Arc<S>) -> Self {
        Self {
            executor,
            transaction_pool,
            storage,
        }
    }
}

/// Help functions for rpc APIs.
impl<E, T, S> AppState<E, T, S>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
{
    pub async fn get_height(&self, number: String) -> RpcResult<u64> {
        match &number.to_ascii_lowercase()[..] {
            "earliest" => Ok(0),
            "latest" | "pending" | "" => {
                let b = await!(self.storage.get_latest_block(Context::new()).compat())?;
                Ok(b.header.height)
            }
            x => {
                let h = util::clean_0x(x);
                Ok(u64::from_str_radix(h, 16).map_err(|e| RpcError::Str(format!("{:?}", e)))?)
            }
        }
    }

    pub async fn get_block(&self, number: String) -> RpcResult<Block> {
        let h = await!(self.get_height(number))?;
        let b = await!(self.storage.get_block_by_height(Context::new(), h).compat())?;
        Ok(b)
    }
}

/// Async rpc APIs.
impl<E, T, S> AppState<E, T, S>
where
    E: Executor,
    T: TransactionPool,
    S: Storage,
{
    pub async fn block_number(&self) -> RpcResult<u64> {
        let b = await!(self.storage.get_latest_block(Context::new()).compat())?;
        Ok(b.header.height)
    }

    pub async fn get_balance(&self, number: String, addr: Address) -> RpcResult<U256> {
        let b = await!(self.get_block(number))?;
        let balance = self
            .executor
            .get_balance(Context::new(), &b.header.state_root, &addr)?;
        Ok(balance)
    }

    pub async fn send_raw_transaction(&self, signed_data: Vec<u8>) -> RpcResult<TxResponse> {
        let ser_untx = await!(AsyncCodec::decode::<cita::UnverifiedTransaction>(
            signed_data
        ))?;
        if ser_untx.transaction.is_none() {
            return Err(RpcError::Str("Transaction not found!".into()));
        };
        let ser_raw_tx = await!(AsyncCodec::encode(ser_untx.clone().transaction.unwrap()))?;
        let message = Hash::from_fixed_bytes(tiny_keccak::keccak256(&ser_raw_tx));
        let untx: core_types::transaction::UnverifiedTransaction = ser_untx.into();
        log::debug!("Accept {:?}", untx);
        let r = await!(self
            .transaction_pool
            .insert(Context::new(), message, untx)
            .compat());
        let r = match r {
            Ok(ok) => ok,
            Err(e) => {
                log::warn!("Insert to pool failed. {:?}", e);
                return Err(e.into());
            }
        };
        Ok(TxResponse::new(r.hash, String::from("OK")))
    }
}
