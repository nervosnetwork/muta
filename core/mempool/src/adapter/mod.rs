pub mod message;
mod rlp_types;

use message::{MsgNewTxs, MsgPullTxs, MsgPushTxs, END_GOSSIP_NEW_TXS, END_RPC_PULL_TXS};
use rlp_types::RlpSignedTransaction;

use std::{
    marker::PhantomData,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use bytes::Bytes;
use common_crypto::Crypto;
use protocol::{
    traits::{Context, Gossip, MemPoolAdapter, Priority, Rpc, Storage},
    types::{Hash, SignedTransaction},
    ProtocolResult,
};

use crate::MemPoolError;

pub struct DefaultMemPoolAdapter<C, N, S> {
    network: N,
    storage: Arc<S>,

    timeout_gap: AtomicU64,

    pin_c: PhantomData<C>,
}

impl<C, N, S> DefaultMemPoolAdapter<C, N, S>
where
    C: Crypto,
    N: Rpc + Gossip,
    S: Storage,
{
    pub fn new(network: N, storage: Arc<S>, timeout_gap: u64) -> Self {
        DefaultMemPoolAdapter {
            network,
            storage,
            timeout_gap: AtomicU64::new(timeout_gap),

            pin_c: PhantomData,
        }
    }
}

#[async_trait]
impl<C, N, S> MemPoolAdapter for DefaultMemPoolAdapter<C, N, S>
where
    C: Crypto + Send + Sync + 'static,
    N: Rpc + Gossip + 'static,
    S: Storage + 'static,
{
    async fn pull_txs(
        &self,
        ctx: Context,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let pull_msg = MsgPullTxs { hashes: tx_hashes };

        let resp_msg = self
            .network
            .call::<MsgPullTxs, MsgPushTxs>(ctx, END_RPC_PULL_TXS, pull_msg, Priority::High)
            .await?;

        Ok(resp_msg.sig_txs)
    }

    async fn broadcast_tx(&self, ctx: Context, stx: SignedTransaction) -> ProtocolResult<()> {
        let gossip_msg = MsgNewTxs { stx };

        self.network
            .broadcast(ctx, END_GOSSIP_NEW_TXS, gossip_msg, Priority::Normal)
            .await
    }

    async fn check_signature(&self, _ctx: Context, tx: SignedTransaction) -> ProtocolResult<()> {
        let hash = tx.tx_hash.as_bytes();
        let pub_key = tx.pubkey.as_ref();
        let sig = tx.signature.as_ref();

        C::verify_signature(hash.as_ref(), sig, pub_key).map_err(|_| {
            MemPoolError::CheckSig {
                tx_hash: tx.tx_hash,
            }
            .into()
        })
    }

    // TODO: Verify Fee?
    // TODO: Verify Nonce?
    // TODO: Cycle limit?
    async fn check_transaction(&self, _ctx: Context, stx: SignedTransaction) -> ProtocolResult<()> {
        // Verify transaction hash
        let rlp_stx = rlp::encode(&RlpSignedTransaction::from(&stx));
        let stx_hash = Hash::digest(Bytes::from(rlp_stx));

        if stx_hash != stx.tx_hash {
            let wrong_hash = MemPoolError::CheckHash {
                expect: stx.tx_hash,
                actual: stx_hash,
            };

            return Err(wrong_hash.into());
        }

        // Verify chain id
        let latest_epoch = self.storage.get_latest_epoch().await?;
        if latest_epoch.header.chain_id != stx.raw.chain_id {
            let wrong_chain_id = MemPoolError::WrongChain {
                tx_hash: stx.tx_hash,
            };

            return Err(wrong_chain_id.into());
        }

        // Verify timeout
        let latest_epoch_id = latest_epoch.header.epoch_id;
        let timeout_gap = self.timeout_gap.load(Ordering::SeqCst);

        if stx.raw.timeout > latest_epoch_id + timeout_gap {
            let invalid_timeout = MemPoolError::InvalidTimeout {
                tx_hash: stx.tx_hash,
            };

            return Err(invalid_timeout.into());
        }

        if stx.raw.timeout < latest_epoch_id {
            let timeout = MemPoolError::Timeout {
                tx_hash: stx.tx_hash,
                timeout: stx.raw.timeout,
            };

            return Err(timeout.into());
        }

        Ok(())
    }

    async fn check_storage_exist(&self, _ctx: Context, tx_hash: Hash) -> ProtocolResult<()> {
        match self.storage.get_transaction_by_hash(tx_hash.clone()).await {
            Ok(_) => Err(MemPoolError::CommittedTx { tx_hash }.into()),
            Err(err) => {
                // TODO: downcast to StorageError
                if err.to_string().contains("get none") {
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }
}
