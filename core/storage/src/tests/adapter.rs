use protocol::traits::{StorageAdapter, StorageBatchModify};
use protocol::types::Hash;

use crate::adapter::memory::MemoryAdapter;
use crate::adapter::rocks::RocksAdapter;
use crate::tests::{get_random_bytes, mock_signed_tx};
use crate::{CommonHashKey, TransactionSchema};

#[tokio::test]
async fn test_adapter_insert() {
    adapter_insert_test(MemoryAdapter::new()).await;
    adapter_insert_test(RocksAdapter::new("rocksdb/test_adapter_insert".to_string(), 64).unwrap())
        .await
}

#[tokio::test]
async fn test_adapter_batch_modify() {
    adapter_batch_modify_test(MemoryAdapter::new()).await;
    adapter_batch_modify_test(
        RocksAdapter::new("rocksdb/test_adapter_batch_modify".to_string(), 64).unwrap(),
    )
    .await
}

#[tokio::test]
async fn test_adapter_remove() {
    adapter_remove_test(MemoryAdapter::new()).await;
    adapter_remove_test(RocksAdapter::new("rocksdb/test_adapter_remove".to_string(), 64).unwrap())
        .await
}

async fn adapter_insert_test(db: impl StorageAdapter) {
    let tx_hash = Hash::digest(get_random_bytes(10));
    let tx_key = CommonHashKey::new(1, tx_hash.clone());
    let stx = mock_signed_tx(tx_hash.clone());

    db.insert::<TransactionSchema>(tx_key.clone(), stx.clone())
        .await
        .unwrap();
    let stx = db.get::<TransactionSchema>(tx_key).await.unwrap().unwrap();

    assert_eq!(tx_hash, stx.tx_hash);
}

async fn adapter_batch_modify_test(db: impl StorageAdapter) {
    let mut stxs = Vec::new();
    let mut keys = Vec::new();
    let mut inserts = Vec::new();

    for _ in 0..10 {
        let tx_hash = Hash::digest(get_random_bytes(10));
        keys.push(CommonHashKey::new(1, tx_hash.clone()));
        let stx = mock_signed_tx(tx_hash);
        stxs.push(stx.clone());
        inserts.push(StorageBatchModify::Insert::<TransactionSchema>(stx));
    }

    db.batch_modify::<TransactionSchema>(keys.clone(), inserts)
        .await
        .unwrap();
    let opt_stxs = db.get_batch::<TransactionSchema>(keys).await.unwrap();

    for i in 0..10 {
        assert_eq!(
            stxs.get(i).unwrap().tx_hash,
            opt_stxs.get(i).unwrap().as_ref().unwrap().tx_hash
        );
    }
}

async fn adapter_remove_test(db: impl StorageAdapter) {
    let tx_hash = Hash::digest(get_random_bytes(10));
    let tx_key = CommonHashKey::new(1, tx_hash.clone());
    let is_exist = db
        .contains::<TransactionSchema>(tx_key.clone())
        .await
        .unwrap();
    assert!(!is_exist);

    let stx = &mock_signed_tx(tx_hash);
    db.insert::<TransactionSchema>(tx_key.clone(), stx.clone())
        .await
        .unwrap();
    let is_exist = db
        .contains::<TransactionSchema>(tx_key.clone())
        .await
        .unwrap();
    assert!(is_exist);

    db.remove::<TransactionSchema>(tx_key.clone())
        .await
        .unwrap();
    let is_exist = db.contains::<TransactionSchema>(tx_key).await.unwrap();
    assert!(!is_exist);
}
