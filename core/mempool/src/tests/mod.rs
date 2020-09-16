extern crate test;

mod mempool;

use std::convert::{From, TryFrom};
use std::sync::Arc;

use async_trait::async_trait;
use chashmap::CHashMap;
use futures::executor;
use rand::random;
use rand::rngs::OsRng;

use common_crypto::{
    Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey, Secp256k1PublicKey,
    Secp256k1Signature, Signature, ToPublicKey,
};
use protocol::codec::ProtocolCodec;
use protocol::traits::{Context, MemPool, MemPoolAdapter, MixedTxHashes};
use protocol::types::{Address, Hash, RawTransaction, SignedTransaction, TransactionRequest};
use protocol::{Bytes, ProtocolResult};

use crate::{check_dup_order_hashes, HashMemPool, MemPoolError};

const CYCLE_LIMIT: u64 = 1_000_000;
const TX_NUM_LIMIT: u64 = 10_000;
const CURRENT_HEIGHT: u64 = 999;
const POOL_SIZE: usize = 100_000;
const MAX_TX_SIZE: u64 = 1024; // 1KB
const TIMEOUT: u64 = 1000;
const TIMEOUT_GAP: u64 = 100;
const TX_CYCLE: u64 = 1;

pub struct HashMemPoolAdapter {
    network_txs: CHashMap<Hash, SignedTransaction>,
}

impl HashMemPoolAdapter {
    fn new() -> HashMemPoolAdapter {
        HashMemPoolAdapter {
            network_txs: CHashMap::new(),
        }
    }
}

#[async_trait]
impl MemPoolAdapter for HashMemPoolAdapter {
    async fn pull_txs(
        &self,
        _ctx: Context,
        _height: Option<u64>,
        tx_hashes: Vec<Hash>,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let mut vec = Vec::new();
        for hash in tx_hashes {
            if let Some(tx) = self.network_txs.get(&hash) {
                vec.push(tx.clone());
            }
        }
        Ok(vec)
    }

    async fn broadcast_tx(&self, _ctx: Context, tx: SignedTransaction) -> ProtocolResult<()> {
        self.network_txs.insert(tx.tx_hash.clone(), tx);
        Ok(())
    }

    async fn check_authorization(
        &self,
        _ctx: Context,
        tx: Box<SignedTransaction>,
    ) -> ProtocolResult<()> {
        check_hash(&tx.clone()).await?;
        check_sig(&tx)
    }

    async fn check_transaction(
        &self,
        _ctx: Context,
        _tx: &SignedTransaction,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    async fn check_storage_exist(&self, _ctx: Context, _tx_hash: &Hash) -> ProtocolResult<()> {
        Ok(())
    }

    async fn get_latest_height(&self, _ctx: Context) -> ProtocolResult<u64> {
        Ok(CURRENT_HEIGHT)
    }

    async fn get_transactions_from_storage(
        &self,
        _ctx: Context,
        _height: Option<u64>,
        _tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<Option<SignedTransaction>>> {
        Ok(vec![])
    }

    fn report_good(&self, _ctx: Context) {}

    fn set_args(&self, _timeout_gap: u64, _cycles_limit: u64, _max_tx_size: u64) {}
}

pub fn default_mock_txs(size: usize) -> Vec<SignedTransaction> {
    mock_txs(size, 0, TIMEOUT)
}

fn mock_txs(valid_size: usize, invalid_size: usize, timeout: u64) -> Vec<SignedTransaction> {
    let mut vec = Vec::new();
    let priv_key = Secp256k1PrivateKey::generate(&mut OsRng);
    let pub_key = priv_key.pub_key();
    for i in 0..valid_size + invalid_size {
        vec.push(mock_signed_tx(&priv_key, &pub_key, timeout, i < valid_size));
    }
    vec
}

fn default_mempool_sync() -> HashMemPool<HashMemPoolAdapter> {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(default_mempool())
}

async fn default_mempool() -> HashMemPool<HashMemPoolAdapter> {
    new_mempool(POOL_SIZE, TIMEOUT_GAP, CYCLE_LIMIT, MAX_TX_SIZE).await
}

async fn new_mempool(
    pool_size: usize,
    timeout_gap: u64,
    cycles_limit: u64,
    max_tx_size: u64,
) -> HashMemPool<HashMemPoolAdapter> {
    let adapter = HashMemPoolAdapter::new();
    let mempool = HashMemPool::new(pool_size, adapter, vec![]).await;
    mempool.set_args(timeout_gap, cycles_limit, max_tx_size);
    mempool
}

async fn check_hash(tx: &SignedTransaction) -> ProtocolResult<()> {
    let mut raw = tx.raw.clone();
    let raw_bytes = raw.encode().await?;
    let tx_hash = Hash::digest(raw_bytes);
    if tx_hash != tx.tx_hash {
        return Err(MemPoolError::CheckHash {
            expect: tx.tx_hash.clone(),
            actual: tx_hash,
        }
        .into());
    }
    Ok(())
}

fn check_sig(tx: &SignedTransaction) -> ProtocolResult<()> {
    if Secp256k1::verify_signature(&tx.tx_hash.as_bytes(), &tx.signature, &tx.pubkey).is_err() {
        return Err(MemPoolError::CheckAuthorization {
            tx_hash:  tx.tx_hash.clone(),
            err_info: "".to_string(),
        }
        .into());
    }
    Ok(())
}

async fn concurrent_check_sig(txs: Vec<SignedTransaction>) {
    let futs = txs
        .into_iter()
        .map(|tx| tokio::task::spawn_blocking(move || check_sig(&tx).unwrap()))
        .collect::<Vec<_>>();

    futures::future::try_join_all(futs).await.unwrap();
}

async fn concurrent_insert(
    txs: Vec<SignedTransaction>,
    mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
) {
    let futs = txs
        .into_iter()
        .map(|tx| {
            let mempool = Arc::clone(&mempool);
            tokio::spawn(async { exec_insert(tx, mempool).await })
        })
        .collect::<Vec<_>>();

    futures::future::try_join_all(futs).await.unwrap();
}

async fn concurrent_broadcast(
    txs: Vec<SignedTransaction>,
    mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
) {
    let futs = txs
        .into_iter()
        .map(|tx| {
            let mempool = Arc::clone(&mempool);
            tokio::spawn(async move {
                mempool
                    .get_adapter()
                    .broadcast_tx(Context::new(), tx)
                    .await
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();

    futures::future::try_join_all(futs).await.unwrap();
}

async fn exec_insert(signed_tx: SignedTransaction, mempool: Arc<HashMemPool<HashMemPoolAdapter>>) {
    let _ = mempool.insert(Context::new(), signed_tx).await.is_ok();
}

async fn exec_flush(remove_hashes: Vec<Hash>, mempool: Arc<HashMemPool<HashMemPoolAdapter>>) {
    mempool.flush(Context::new(), &remove_hashes).await.unwrap()
}

async fn exec_package(
    mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
    cycle_limit: u64,
    tx_num_limit: u64,
) -> MixedTxHashes {
    mempool
        .package(Context::new(), cycle_limit, tx_num_limit)
        .await
        .unwrap()
}

async fn exec_ensure_order_txs(
    require_hashes: Vec<Hash>,
    mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
) {
    mempool
        .ensure_order_txs(Context::new(), None, &require_hashes)
        .await
        .unwrap();
}

async fn exec_sync_propose_txs(
    require_hashes: Vec<Hash>,
    mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
) {
    mempool
        .sync_propose_txs(Context::new(), require_hashes)
        .await
        .unwrap();
}

async fn exec_get_full_txs(
    require_hashes: Vec<Hash>,
    mempool: Arc<HashMemPool<HashMemPoolAdapter>>,
) -> Vec<SignedTransaction> {
    mempool
        .get_full_txs(Context::new(), None, &require_hashes)
        .await
        .unwrap()
}

fn mock_signed_tx(
    priv_key: &Secp256k1PrivateKey,
    pub_key: &Secp256k1PublicKey,
    timeout: u64,
    valid: bool,
) -> SignedTransaction {
    let nonce = Hash::digest(Bytes::from(get_random_bytes(10)));

    let request = TransactionRequest {
        service_name: "test".to_owned(),
        method:       "test".to_owned(),
        payload:      "test".to_owned(),
    };
    let mut raw = RawTransaction {
        chain_id: nonce.clone(),
        nonce,
        timeout,
        cycles_limit: TX_CYCLE,
        cycles_price: 1,
        request,
        sender: Address::from_pubkey_bytes(pub_key.to_bytes()).unwrap(),
    };

    let raw_bytes = executor::block_on(async { raw.encode().await.unwrap() });
    let tx_hash = Hash::digest(raw_bytes);

    let signature = if valid {
        Secp256k1::sign_message(&tx_hash.as_bytes(), &priv_key.to_bytes()).unwrap()
    } else {
        Secp256k1Signature::try_from([0u8; 64].as_ref()).unwrap()
    };

    SignedTransaction {
        raw,
        tx_hash,
        pubkey: pub_key.to_bytes(),
        signature: signature.to_bytes(),
    }
}

fn get_random_bytes(len: usize) -> Vec<u8> {
    (0..len).map(|_| random::<u8>()).collect()
}

fn check_order_consistant(mixed_tx_hashes: &MixedTxHashes, txs: &[SignedTransaction]) -> bool {
    mixed_tx_hashes
        .order_tx_hashes
        .iter()
        .enumerate()
        .any(|(i, hash)| hash == &txs.get(i).unwrap().tx_hash)
}
