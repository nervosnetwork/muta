use crate::types::Filter;
use core_storage::storage::Storage;
use core_types::{Block, BloomRef, Hash, TransactionPosition};
use futures::future::{err, join_all, ok, Future};
use jsonrpc_core::{BoxFuture, Error as JsonrpcError};
use jsonrpc_types::rpctypes::{BlockNumber, BlockTag, Data, Data32, Log, Quantity};
use log::error;
use std::sync::Arc;

pub fn transform_data32_to_hash(hash: Data32) -> Hash {
    Hash::from_bytes(&Into::<Vec<u8>>::into(hash)).expect("never returns an error")
}

pub fn get_block_by_tx_hash<S>(storage: Arc<S>, tx_hash: &Hash) -> BoxFuture<Option<Block>>
where
    S: Storage + 'static,
{
    let fut = storage
        .get_transaction_position(ctx, &tx_hash)
        .map_err(|e| {
            error!("get_transaction_position err: {:?}", e);
            JsonrpcError::internal_error()
        })
        .and_then(move |position| match position {
            None => ok(None),
            Some(TransactionPosition { block_hash, .. }) => {
                match storage.get_block_by_hash(ctx, &block_hash).wait() {
                    Ok(b) => ok(b),
                    Err(e) => {
                        error!("unexpected get_block_by_height err: {:?}", e);
                        err(JsonrpcError::internal_error())
                    }
                }
            }
        });
    Box::new(fut)
}

pub fn get_current_height<S>(storage: Arc<S>) -> BoxFuture<Quantity>
where
    S: Storage,
{
    let fut = storage
        .get_latest_block(ctx)
        .map_err(|e| {
            error!("get_latest_block err: {:?}", e);
            JsonrpcError::internal_error()
        })
        .and_then(|block| ok(Quantity::new(block.header.height.into())));

    Box::new(fut)
}

pub fn get_block_by_block_number<S>(
    storage: Arc<S>,
    block_number: BlockNumber,
) -> BoxFuture<Option<Block>>
where
    S: Storage + 'static,
{
    let fut = get_height_by_block_number(Arc::<S>::clone(&storage), block_number).and_then(
        move |height| {
            Arc::<S>::clone(&storage)
                .get_block_by_height(ctx, height)
                .then(move |x| {
                    let res: BoxFuture<_> = match x {
                        Ok(block) => match block {
                            Some(blk) => Box::new(ok(Some(blk))),
                            None => Box::new(ok(None)),
                        },
                        Err(e) => {
                            error!("get_block err: {:?}", e);
                            Box::new(err(JsonrpcError::internal_error()))
                        }
                    };
                    res
                })
        },
    );
    Box::new(fut)
}

pub fn get_height_by_block_number<S>(storage: Arc<S>, block_number: BlockNumber) -> BoxFuture<u64>
where
    S: Storage,
{
    match block_number {
        BlockNumber::Height(q) => Box::new(ok(q.into())),
        BlockNumber::Tag(tag) => match tag {
            BlockTag::Earliest => Box::new(ok(0)),
            // TODO: make the concept of latest and pending clear
            BlockTag::Latest | BlockTag::Pending => Box::new(
                storage
                    .get_latest_block(ctx)
                    .map_err(|e| {
                        error!("get_latest_block err: {:?}", e);
                        JsonrpcError::internal_error()
                    })
                    .and_then(|block| ok(block.header.height)),
            ),
        },
    }
}

pub fn get_logs<S>(storage: Arc<S>, filter: Filter) -> BoxFuture<Vec<Log>>
where
    S: Storage + 'static,
{
    let possible_blooms = filter.bloom_possibilities();
    let storage1 = Arc::clone(&storage);
    let storage2 = Arc::clone(&storage);
    let from_block_fut =
        get_height_by_block_number(Arc::clone(&storage), filter.from_block.clone());
    let to_block_fut = get_height_by_block_number(Arc::clone(&storage), filter.to_block.clone());

    let fut = join_all(vec![from_block_fut, to_block_fut])
        .and_then(move |from_to| {
            join_all((from_to[0]..=from_to[1]).map(move |height| {
                storage1.get_block_by_height(ctx, height).map_err(|e| {
                    error!("get_block_by_height err: {:?}", e);
                    JsonrpcError::internal_error()
                })
            }))
        })
        .and_then(move |blocks| {
            let filtered_blocks = blocks
                .into_iter()
                .filter(|blk| {
                    match blk {
                        Some(block) => possible_blooms.iter().any(|bloom| {
                            bloom.contains_bloom(BloomRef::from(&block.header.logs_bloom))
                        }),
                        // TODO: raise error?
                        None => false,
                    }
                })
                .map(std::option::Option::unwrap)
                .collect::<Vec<_>>();

            let tx_hashes = filtered_blocks
                .iter()
                .flat_map(|block| block.tx_hashes.iter())
                .collect::<Vec<_>>();
            if tx_hashes.is_empty() {
                return ok(vec![]);
            }
            let receipts_res = storage2.get_receipts(ctx, tx_hashes.as_slice()).wait();
            match receipts_res {
                Err(e) => {
                    error!("get_receipts err: {:?}", e);
                    err(JsonrpcError::internal_error())
                }
                Ok(receipts) => {
                    let mut logs = vec![];
                    let mut tx_idx = 0;
                    for block in &filtered_blocks {
                        let mut log_index = 0;
                        for tx_hash in &block.tx_hashes {
                            let mut _transaction_index = 0;
                            match &receipts[tx_idx] {
                                None => {
                                    error! {"can not get receipt for {:?}", tx_hashes[tx_idx]};
                                    return err(JsonrpcError::internal_error());
                                }
                                Some(receipt) => {
                                    let receipt_contains_bloom =
                                        possible_blooms.iter().any(|bloom| {
                                            bloom
                                                .contains_bloom(BloomRef::from(&receipt.logs_bloom))
                                        });
                                    if receipt_contains_bloom {
                                        for log_entry in &receipt.logs {
                                            let mut _transaction_log_index = 0;
                                            if filter.matches(&log_entry) {
                                                let log = Log {
                                                    address: log_entry.address.as_bytes().into(),
                                                    topics: log_entry
                                                        .topics
                                                        .iter()
                                                        .map(|t| t.as_bytes().into())
                                                        .collect(),
                                                    data: Data::new(log_entry.data.clone()),
                                                    block_hash: Some(
                                                        block.header.hash().as_bytes().into(),
                                                    ),
                                                    block_number: Some(block.header.height.into()),
                                                    transaction_hash: Some(
                                                        tx_hash.as_bytes().into(),
                                                    ),
                                                    transaction_index: Some(
                                                        _transaction_index.into(),
                                                    ),
                                                    log_index: Some(log_index.into()),
                                                    transaction_log_index: Some(
                                                        _transaction_log_index.into(),
                                                    ),
                                                };
                                                logs.push(log);
                                            }
                                            _transaction_log_index += 1;
                                            log_index += 1;
                                        }
                                    } else {
                                        log_index += &receipt.logs.len();
                                    }
                                }
                            }
                            _transaction_index += 1;
                            tx_idx += 1;
                        }
                    }
                    let len = logs.len();
                    let logs = match filter.limit {
                        Some(limit) if len >= limit => logs.split_off(len - limit),
                        _ => logs,
                    };
                    ok(logs)
                }
            }
        });

    Box::new(fut)
}
