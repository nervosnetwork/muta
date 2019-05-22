use std::collections::HashMap;
use std::convert::TryInto;
use std::iter::FromIterator;
use std::sync::Arc;

use byteorder::{ByteOrder, NativeEndian};
use futures::{
    compat::Future01CompatExt,
    prelude::{FutureExt, TryFutureExt, TryStreamExt},
    stream::FuturesOrdered,
};

use core_context::Context;
use core_runtime::{DataCategory, Database, Storage, StorageResult};
use core_serialization::{
    AsyncCodec, Block as SerBlock, Proof as SerProof, Receipt as SerReceipt,
    SignedTransaction as SerSignedTransaction, TransactionPosition as SerTransactionPosition,
};
use core_types::{Block, Hash, Proof, Receipt, SignedTransaction, TransactionPosition};

const LATEST_BLOCK: &[u8] = b"latest-block";
const LATEST_PROOF: &[u8] = b"latest-proof";

pub struct BlockStorage<DB>
where
    DB: Database,
{
    db: Arc<DB>,
}

impl<DB> BlockStorage<DB>
where
    DB: Database,
{
    pub fn new(db: Arc<DB>) -> Self {
        BlockStorage { db }
    }
}

impl<DB: 'static> Storage for BlockStorage<DB>
where
    DB: Database,
{
    fn get_latest_block(&self, ctx: Context) -> StorageResult<Block> {
        let db = Arc::clone(&self.db);

        let fut = async move {
            let value = db
                .get(ctx, DataCategory::Block, LATEST_BLOCK)
                .compat()
                .await?;

            let block = AsyncCodec::decode::<SerBlock>(value?).await?.try_into()?;
            Ok(block)
        };

        Box::new(fut.boxed().compat())
    }

    fn get_block_by_height(&self, ctx: Context, height: u64) -> StorageResult<Block> {
        let db = Arc::clone(&self.db);
        let key = transfrom_u64_to_array_u8(height);

        let fut = async move {
            let value = db.get(ctx, DataCategory::Block, &key).compat().await?;

            let block = AsyncCodec::decode::<SerBlock>(value?).await?.try_into()?;
            Ok(block)
        };

        Box::new(fut.boxed().compat())
    }

    fn get_block_by_hash(&self, ctx: Context, hash: &Hash) -> StorageResult<Block> {
        let db = Arc::clone(&self.db);
        let key = hash.clone();

        let fut = async move {
            let height_slice = db
                .get(ctx.clone(), DataCategory::Block, key.as_bytes())
                .compat()
                .await?;
            let value = db
                .get(ctx, DataCategory::Block, &height_slice?)
                .compat()
                .await?;

            let block = AsyncCodec::decode::<SerBlock>(value?).await?.try_into()?;
            Ok(block)
        };

        Box::new(fut.boxed().compat())
    }

    fn get_transaction(&self, ctx: Context, hash: &Hash) -> StorageResult<SignedTransaction> {
        let db = Arc::clone(&self.db);
        let key = hash.clone();

        let fut = async move {
            let value = db
                .get(ctx, DataCategory::Transaction, key.as_bytes())
                .compat()
                .await?;

            let tx = AsyncCodec::decode::<SerSignedTransaction>(value?)
                .await?
                .try_into()?;
            Ok(tx)
        };

        Box::new(fut.boxed().compat())
    }

    fn get_transactions(
        &self,
        ctx: Context,
        hashes: &[Hash],
    ) -> StorageResult<Vec<SignedTransaction>> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = hashes.iter().map(|h| h.as_bytes().to_vec()).collect();

        let fut = async move {
            let values = db
                .get_batch(ctx, DataCategory::Transaction, &keys)
                .compat()
                .await?;
            let values = opts_to_flat(values);

            let txs = AsyncCodec::decode_batch::<SerSignedTransaction>(values)
                .await?
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<SignedTransaction>, _>>()?;
            Ok(txs)
        };

        Box::new(fut.boxed().compat())
    }

    fn get_receipt(&self, ctx: Context, hash: &Hash) -> StorageResult<Receipt> {
        let db = Arc::clone(&self.db);
        let key = hash.clone();

        let fut = async move {
            let value = db
                .get(ctx, DataCategory::Receipt, key.as_bytes())
                .compat()
                .await?;

            let receipt = AsyncCodec::decode::<SerReceipt>(value?).await?.try_into()?;
            Ok(receipt)
        };

        Box::new(fut.boxed().compat())
    }

    fn get_receipts(&self, ctx: Context, hashes: &[Hash]) -> StorageResult<Vec<Receipt>> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = hashes.iter().map(|h| h.as_bytes().to_vec()).collect();

        let fut = async move {
            let values = db
                .get_batch(ctx, DataCategory::Receipt, &keys)
                .compat()
                .await?;
            let values = opts_to_flat(values);

            let receipts = AsyncCodec::decode_batch::<SerReceipt>(values)
                .await?
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<Receipt>, _>>()?;
            Ok(receipts)
        };

        Box::new(fut.boxed().compat())
    }

    fn get_transaction_position(
        &self,
        ctx: Context,
        hash: &Hash,
    ) -> StorageResult<TransactionPosition> {
        let db = Arc::clone(&self.db);
        let key = hash.clone();

        let fut = async move {
            let value = db
                .get(ctx, DataCategory::TransactionPosition, key.as_bytes())
                .compat()
                .await?;

            let tx_position = AsyncCodec::decode::<SerTransactionPosition>(value?)
                .await?
                .try_into()?;
            Ok(tx_position)
        };

        Box::new(fut.boxed().compat())
    }

    fn get_transaction_positions(
        &self,
        ctx: Context,
        hashes: &[Hash],
    ) -> StorageResult<Vec<TransactionPosition>> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = hashes.iter().map(|h| h.as_bytes().to_vec()).collect();

        let fut = async move {
            let values = db
                .get_batch(ctx, DataCategory::TransactionPosition, &keys)
                .compat()
                .await?;
            let values = opts_to_flat(values);

            let positions = AsyncCodec::decode_batch::<SerTransactionPosition>(values)
                .await?
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<TransactionPosition>, _>>()?;
            Ok(positions)
        };

        Box::new(fut.boxed().compat())
    }

    fn get_latest_proof(&self, ctx: Context) -> StorageResult<Proof> {
        let db = Arc::clone(&self.db);

        let fut = async move {
            let value = db
                .get(ctx, DataCategory::Block, &LATEST_PROOF.to_vec())
                .compat()
                .await?;
            let proof: Proof = AsyncCodec::decode::<SerProof>(value?).await?.try_into()?;
            Ok(proof)
        };

        Box::new(fut.boxed().compat())
    }

    fn insert_block(&self, ctx: Context, block: Block) -> StorageResult<()> {
        let db = Arc::clone(&self.db);

        let height = block.header.height;
        let height_key = transfrom_u64_to_array_u8(block.header.height);
        let hash_key = block.hash.clone();

        let pb_block: SerBlock = block.into();

        let fut = async move {
            let encode_value = AsyncCodec::encode(pb_block).await?;

            let stream = FuturesOrdered::from_iter(vec![
                db.insert(
                    ctx.clone(),
                    DataCategory::Block,
                    height_key,
                    encode_value.clone(),
                )
                .compat(),
                db.insert(
                    ctx.clone(),
                    DataCategory::Block,
                    hash_key.as_bytes().to_vec(),
                    transfrom_u64_to_array_u8(height),
                )
                .compat(),
                db.insert(
                    ctx,
                    DataCategory::Block,
                    LATEST_BLOCK.to_vec(),
                    encode_value.clone(),
                )
                .compat(),
            ]);

            stream.try_collect().await?;
            Ok(())
        };

        Box::new(fut.boxed().compat())
    }

    fn insert_transactions(
        &self,
        ctx: Context,
        signed_txs: Vec<SignedTransaction>,
    ) -> StorageResult<()> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = signed_txs
            .iter()
            .map(|tx| tx.hash.as_bytes().to_vec())
            .collect();

        let fut = async move {
            let pb_txs: Vec<SerSignedTransaction> =
                signed_txs.into_iter().map(Into::into).collect();
            let values = AsyncCodec::encode_batch(pb_txs).await?;

            db.insert_batch(ctx, DataCategory::Transaction, keys, values)
                .compat()
                .await?;
            Ok(())
        };

        Box::new(fut.boxed().compat())
    }

    fn insert_transaction_positions(
        &self,
        ctx: Context,
        positions: HashMap<Hash, TransactionPosition>,
    ) -> StorageResult<()> {
        let db = Arc::clone(&self.db);

        let fut = async move {
            let mut keys: Vec<Vec<u8>> = Vec::with_capacity(positions.len());
            let mut ser_positions: Vec<SerTransactionPosition> =
                Vec::with_capacity(positions.len());

            for (key, position) in positions.into_iter() {
                keys.push(key.as_bytes().to_vec());
                ser_positions.push(position.into());
            }

            let values = AsyncCodec::encode_batch(ser_positions).await?;

            db.insert_batch(ctx, DataCategory::TransactionPosition, keys, values)
                .compat()
                .await?;
            Ok(())
        };

        Box::new(fut.boxed().compat())
    }

    fn insert_receipts(&self, ctx: Context, receipts: Vec<Receipt>) -> StorageResult<()> {
        let db = Arc::clone(&self.db);
        let keys: Vec<Vec<u8>> = receipts
            .iter()
            .map(|r| r.transaction_hash.as_bytes().to_vec())
            .collect();

        let fut = async move {
            let pb_receipts: Vec<SerReceipt> = receipts.into_iter().map(Into::into).collect();
            let values = AsyncCodec::encode_batch(pb_receipts).await?;

            db.insert_batch(ctx, DataCategory::Receipt, keys, values)
                .compat()
                .await?;
            Ok(())
        };

        Box::new(fut.boxed().compat())
    }

    fn update_latest_proof(&self, ctx: Context, proof: Proof) -> StorageResult<()> {
        let db = Arc::clone(&self.db);

        let fut = async move {
            let value = AsyncCodec::encode::<SerProof>(proof.into()).await?;
            db.insert(ctx, DataCategory::Block, LATEST_PROOF.to_vec(), value)
                .compat()
                .await?;
            Ok(())
        };

        Box::new(fut.boxed().compat())
    }
}

fn transfrom_u64_to_array_u8(n: u64) -> Vec<u8> {
    let mut u64_slice = [0u8; 8];
    NativeEndian::write_u64(&mut u64_slice, n);
    u64_slice.to_vec()
}

fn opts_to_flat(values: Vec<Option<Vec<u8>>>) -> Vec<Vec<u8>> {
    values
        .into_iter()
        .filter(Option::is_some)
        .map(|v| v.expect("get value"))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use old_futures::future::Future;

    use super::{BlockStorage, Storage};

    use components_database::memory::MemoryDB;
    use core_context::Context;
    use core_types::{
        Block, Hash, Proof, Receipt, SignedTransaction, TransactionPosition, UnverifiedTransaction,
    };

    #[test]
    fn test_get_latest_block_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        storage
            .insert_block(ctx.clone(), mock_block(1000))
            .wait()
            .unwrap();
        let block = storage.get_latest_block(ctx).wait().unwrap();

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_height_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        storage
            .insert_block(ctx.clone(), mock_block(1000))
            .wait()
            .unwrap();
        let block = storage.get_block_by_height(ctx, 1000).wait().unwrap();

        assert_eq!(block.header.height, 1000)
    }

    #[test]
    fn test_get_block_by_hash_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);

        let b = mock_block(1000);
        let hash = b.header.hash().clone();
        storage.insert_block(ctx.clone(), b).wait().unwrap();

        let b = storage.get_block_by_hash(ctx, &hash).wait().unwrap();
        assert_eq!(b.header.height, 1000)
    }

    #[test]
    fn test_get_transaction_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx = mock_transaction(Hash::digest(b"test111"));

        let hash = tx.hash.clone();
        storage
            .insert_transactions(ctx.clone(), vec![tx])
            .wait()
            .unwrap();
        let new_tx = storage.get_transaction(ctx, &hash).wait().unwrap();

        assert_eq!(new_tx.hash, hash)
    }

    #[test]
    fn test_get_transactions_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx1 = mock_transaction(Hash::digest(b"test111"));
        let tx2 = mock_transaction(Hash::digest(b"test222"));

        let tx_hash1 = tx1.hash.clone();
        let tx_hash2 = tx2.hash.clone();
        storage
            .insert_transactions(ctx.clone(), vec![tx1, tx2])
            .wait()
            .unwrap();
        let transactions = storage
            .get_transactions(ctx, &[tx_hash1.clone(), tx_hash2.clone()])
            .wait()
            .unwrap();
        assert_eq!(transactions.len(), 2);

        let hashes: Vec<Hash> = transactions.into_iter().map(|tx| tx.hash).collect();

        assert!(hashes.contains(&tx_hash1));
        assert!(hashes.contains(&tx_hash2));
    }

    #[test]
    fn test_transaction_position_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx_position = mock_transaction_position(Hash::default(), 0);

        let hash = Hash::digest(b"test");
        let mut positions = HashMap::new();
        positions.insert(hash.clone(), tx_position.clone());
        storage
            .insert_transaction_positions(ctx.clone(), positions)
            .wait()
            .unwrap();
        let new_tx_position = storage.get_transaction_position(ctx, &hash).wait().unwrap();

        assert_eq!(new_tx_position, tx_position);
    }

    #[test]
    fn test_get_transaction_positions_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let tx_position1 = mock_transaction_position(Hash::default(), 0);
        let tx_position2 = mock_transaction_position(Hash::default(), 1);

        let hash1 = Hash::digest(b"test");
        let hash2 = Hash::digest(b"test2");

        let mut positions = HashMap::new();
        positions.insert(hash1.clone(), tx_position1.clone());
        positions.insert(hash2.clone(), tx_position2.clone());
        storage
            .insert_transaction_positions(ctx.clone(), positions)
            .wait()
            .unwrap();
        let tx_positions = storage
            .get_transaction_positions(ctx, &[hash1, hash2])
            .wait()
            .unwrap();
        assert_eq!(tx_positions.len(), 2);

        assert!(tx_positions.contains(&tx_position1));
        assert!(tx_positions.contains(&tx_position2));
    }

    #[test]
    fn test_get_receipt_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let receipt = mock_receipt(Hash::digest(b"test111"));
        let tx_hash = receipt.transaction_hash.clone();

        storage
            .insert_receipts(ctx.clone(), vec![receipt])
            .wait()
            .unwrap();
        let receipt = storage.get_receipt(ctx, &tx_hash).wait().unwrap();
        assert_eq!(receipt.transaction_hash, tx_hash);
    }

    #[test]
    fn test_get_receipts_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);
        let receipt1 = mock_receipt(Hash::digest(b"test111"));
        let receipt2 = mock_receipt(Hash::digest(b"test222"));

        let tx_hash1 = receipt1.transaction_hash.clone();
        let tx_hash2 = receipt2.transaction_hash.clone();
        storage
            .insert_receipts(ctx.clone(), vec![receipt1, receipt2])
            .wait()
            .unwrap();
        let transactions = storage
            .get_receipts(ctx, &[tx_hash1.clone(), tx_hash2.clone()])
            .wait()
            .unwrap();
        assert_eq!(transactions.len(), 2);

        let hashes: Vec<Hash> = transactions
            .into_iter()
            .map(|receipt| receipt.transaction_hash)
            .collect();

        assert!(hashes.contains(&tx_hash1));
        assert!(hashes.contains(&tx_hash2));
    }

    #[test]
    fn test_insert_block_should_return_ok() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);

        let block = mock_block(1000);
        let height = block.header.height;
        let hash = block.header.hash().clone();
        storage.insert_block(ctx.clone(), block).wait().unwrap();
        assert_eq!(
            storage
                .get_latest_block(ctx.clone())
                .wait()
                .unwrap()
                .header
                .height,
            height
        );
        assert_eq!(
            storage
                .get_block_by_height(ctx.clone(), height)
                .wait()
                .unwrap()
                .header
                .height,
            height
        );

        assert_eq!(
            storage
                .get_block_by_hash(ctx, &hash)
                .wait()
                .unwrap()
                .header
                .height,
            height
        );
    }

    #[test]
    fn test_insert_proof() {
        let ctx = Context::new();
        let db = Arc::new(MemoryDB::new());
        let storage = BlockStorage::new(db);

        storage
            .update_latest_proof(ctx.clone(), Proof {
                height: 10,
                round: 10,
                ..Default::default()
            })
            .wait()
            .unwrap();

        let proof = storage.get_latest_proof(ctx.clone()).wait().unwrap();
        assert_eq!(proof.height, 10);
        assert_eq!(proof.round, 10);
    }

    fn mock_block(height: u64) -> Block {
        let mut b = Block::default();
        b.header.prevhash = Hash::digest(b"test");
        b.header.timestamp = 1234;
        b.header.height = height;
        b.tx_hashes = vec![Hash::digest(b"tx1"), Hash::digest(b"tx2")];
        b.hash = b.header.hash();
        b
    }

    fn mock_transaction(tx_hash: Hash) -> SignedTransaction {
        let mut signed_tx = SignedTransaction::default();
        signed_tx.hash = tx_hash;
        signed_tx.untx = UnverifiedTransaction::default();
        signed_tx
    }

    fn mock_receipt(tx_hash: Hash) -> Receipt {
        let mut receipt = Receipt::default();
        receipt.transaction_hash = tx_hash;
        receipt
    }

    fn mock_transaction_position(block_hash: Hash, position: u32) -> TransactionPosition {
        TransactionPosition {
            block_hash,
            position,
        }
    }
}
