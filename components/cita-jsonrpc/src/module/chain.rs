use std::sync::Arc;

use futures::future::{err, ok, result, Future};
use log::error;
use rlp;

use core_merkle::{merge, Merkle};
use core_runtime::{Executor, TransactionPool};
use core_serialization::{AsyncCodec, UnverifiedTransaction as SerUnverifiedTransaction};
use core_storage::storage::Storage;
use core_types::{
    Address, Balance, Block as RawBlock, Hash, Receipt as RawReceipt, SignedTransaction, H256,
};
use jsonrpc_core::{BoxFuture, Error as JsonrpcError};
use jsonrpc_derive::rpc;
use jsonrpc_types::rpctypes::{
    Block, BlockBody, BlockHeader, BlockNumber, BlockTransaction, CallRequest, Data, Data20,
    Data32, Filter as RpcFilter, FullTransaction, Log, MetaData, Quantity, Receipt, RpcTransaction,
    TxResponse,
};

use crate::helpers::{
    get_block_by_block_number, get_block_by_tx_hash, get_logs, transform_data32_to_hash,
};
use crate::types::{Filter, ReceiptProof};

#[rpc]
pub trait Chain {
    #[rpc(name = "blockNumber")]
    fn block_number(&self) -> BoxFuture<Quantity>;

    #[rpc(name = "sendRawTransaction")]
    fn send_raw_transaction(&self, signed_data: Data) -> BoxFuture<TxResponse>;

    #[rpc(name = "getBlockByHash")]
    fn get_block_by_hash(&self, hash: Data32, include_tx: bool) -> BoxFuture<Option<Block>>;

    #[rpc(name = "getBlockByNumber")]
    fn get_block_by_number(&self, height: Quantity, include_tx: bool) -> BoxFuture<Option<Block>>;

    #[rpc(name = "getTransactionReceipt")]
    fn get_transaction_receipt(&self, hash: Data32) -> BoxFuture<Receipt>;

    #[rpc(name = "getLogs")]
    fn get_logs(&self, filter: RpcFilter) -> BoxFuture<Vec<Log>>;

    #[rpc(name = "call")]
    fn call(&self, call_request: CallRequest, block_number: BlockNumber) -> BoxFuture<Data>;

    #[rpc(name = "getTransaction")]
    fn get_transaction(&self, hash: Data32) -> BoxFuture<Option<RpcTransaction>>;

    #[rpc(name = "getTransactionCount")]
    fn get_transaction_count(&self, addr: Data20, block_number: BlockNumber)
        -> BoxFuture<Quantity>;

    #[rpc(name = "getCode")]
    fn get_code(&self, addr: Data20, block_number: BlockNumber) -> BoxFuture<Data>;

    // not ready
    #[rpc(name = "getAbi")]
    fn get_abi(&self, addr: Data20, block_number: BlockNumber) -> BoxFuture<Data>;

    #[rpc(name = "getBalance")]
    fn get_balance(&self, addr: Data20, block_number: BlockNumber) -> BoxFuture<Quantity>;

    #[rpc(name = "getTransactionProof")]
    fn get_transaction_proof(&self, hash: Data32) -> BoxFuture<Data>;

    #[rpc(name = "getReceiptProof")]
    fn get_receipt_proof(&self, hash: Data32) -> BoxFuture<Option<Data>>;

    // not ready
    #[rpc(name = "getMetaData")]
    fn get_meta_data(&self, block_number: BlockNumber) -> BoxFuture<MetaData>;

    #[rpc(name = "getBlockHeader")]
    fn get_block_header(&self, block_number: BlockNumber) -> BoxFuture<Data>;

    // not ready
    #[rpc(name = "getStateProof")]
    fn get_state_proof(
        &self,
        addr: Data20,
        key: Data32,
        block_number: BlockNumber,
    ) -> BoxFuture<Data>;

    #[rpc(name = "getStorageAt")]
    fn get_storage_at(
        &self,
        addr: Data20,
        key: Data32,
        block_number: BlockNumber,
    ) -> BoxFuture<Data>;
}

pub struct ChainRpcImpl<S, E, T>
where
    S: Storage,
    E: Executor,
    T: TransactionPool,
{
    storage: Arc<S>,
    executor: Arc<E>,
    transaction_pool: Arc<T>,
}

impl<S, E, T> ChainRpcImpl<S, E, T>
where
    S: Storage + 'static,
    E: Executor + 'static,
    T: TransactionPool + 'static,
{
    pub fn new(storage: Arc<S>, executor: Arc<E>, transaction_pool: Arc<T>) -> Self {
        Self {
            storage,
            executor,
            transaction_pool,
        }
    }

    pub fn get_storage_inst(&self) -> Arc<S> {
        Arc::clone(&self.storage)
    }

    pub fn get_transaction_pool_inst(&self) -> Arc<T> {
        Arc::clone(&self.transaction_pool)
    }

    pub fn get_executor_inst(&self) -> Arc<E> {
        Arc::clone(&self.executor)
    }
}

fn get_jsonrpc_block_from_raw_block<S>(
    storage: Arc<S>,
    raw_block: &RawBlock,
    include_tx: bool,
) -> BoxFuture<Block>
where
    S: Storage,
{
    let mut res_block = Block {
        version: 0,
        hash: raw_block.header.hash().as_bytes().into(),
        header: BlockHeader {
            timestamp: raw_block.header.timestamp,
            prev_hash: raw_block.header.prevhash.as_bytes().into(),
            number: raw_block.header.height.into(),
            state_root: raw_block.header.state_root.as_bytes().into(),
            transactions_root: raw_block.header.transactions_root.as_bytes().into(),
            receipts_root: raw_block.header.receipts_root.as_bytes().into(),
            quota_used: raw_block.header.quota_used.into(),
            proof: None,
            proposer: raw_block.header.proposer.as_bytes().into(),
        },
        body: BlockBody {
            transactions: raw_block
                .tx_hashes
                .iter()
                .map(|tx| BlockTransaction::Hash(tx.as_bytes().into()))
                .collect(),
        },
    };
    if !include_tx {
        return Box::new(ok(res_block));
    }
    let fut = storage
        .get_transactions(ctx, &raw_block.tx_hashes.iter().collect::<Vec<_>>())
        .map_err(|e| {
            error!("get_transactions err: {:?}", e);
            JsonrpcError::internal_error()
        })
        .and_then(|raw_txs| {
            let mut txs = vec![];
            for tx in raw_txs {
                match tx {
                    None => {
                        error!("transaction in header not found err");
                        return err(JsonrpcError::internal_error());
                    }
                    Some(tx) => txs.push(BlockTransaction::Full(FullTransaction {
                        hash: tx.hash.as_bytes().into(),
                        content: tx.untx.transaction.data.into(),
                        from: tx.sender.as_bytes().into(),
                    })),
                }
            }
            ok(txs)
        })
        .map(|txs| {
            res_block.body.transactions = txs;
            res_block
        });
    Box::new(fut)
}

fn get_jsonrpc_tx_from_raw_tx<S>(
    storage: Arc<S>,
    raw_tx: SignedTransaction,
) -> BoxFuture<RpcTransaction>
where
    S: Storage + 'static,
{
    let tx_hash = raw_tx.hash.clone();
    let fut = get_block_by_tx_hash(storage, &tx_hash).map(move |block| {
        let mut tx = RpcTransaction {
            hash: raw_tx.hash.as_bytes().into(),
            content: raw_tx.untx.transaction.data.clone().into(),
            from: raw_tx.sender.as_bytes().into(),
            block_number: 0.into(),
            block_hash: 0.into(),
            index: 0.into(),
        };
        if let Some(b) = block {
            tx.index = b
                .tx_hashes
                .iter()
                .position(|x| x == &tx_hash)
                .unwrap()
                .into();
            tx.block_hash = b.header.hash().as_bytes().into();
            tx.block_number = b.header.height.into();
        };
        tx
    });

    Box::new(fut)
}

fn get_jsonrpc_receipt_from_raw_receipt<S>(
    storage: Arc<S>,
    raw_receipt: RawReceipt,
) -> BoxFuture<Receipt>
where
    S: Storage + 'static,
{
    let storage1 = Arc::clone(&storage);
    let raw_receipt2 = raw_receipt.clone();
    let fut = get_block_by_tx_hash(storage, &raw_receipt.transaction_hash)
        .and_then(|block| match block {
            None => {
                error!("unexpected err, block is not found");
                Err(JsonrpcError::internal_error())
            }
            Some(block) => Ok(block),
        })
        .and_then(move |block| {
            storage1
                .get_receipts(ctx, block.tx_hashes.iter().collect::<Vec<_>>().as_slice())
                .map_err(|e| {
                    error!("get_receipts err: {:?}", e);
                    JsonrpcError::internal_error()
                })
                .and_then(move |receipts| {
                    let mut logs_in_block_before_receipt = 0;
                    let mut tx_index = 0;
                    let tx_hash = raw_receipt2.transaction_hash.clone();
                    for r in receipts {
                        match r {
                            None => {
                                error!("unexpected err, some receipt is none");
                                return err(JsonrpcError::internal_error());
                            }
                            Some(r) => {
                                if r.transaction_hash == tx_hash {
                                    return ok((block, logs_in_block_before_receipt, tx_index));
                                } else {
                                    logs_in_block_before_receipt += r.logs.len();
                                    tx_index += 1;
                                }
                            }
                        }
                    }
                    error!("unexpected err, tx not found in block");
                    err(JsonrpcError::internal_error())
                })
                .and_then(move |(block, logs_in_block_before_receipt, tx_index)| {
                    let logs = raw_receipt
                        .logs
                        .iter()
                        .enumerate()
                        .map(|(log_index, log_entry)| Log {
                            address: log_entry.address.as_bytes().into(),
                            topics: log_entry
                                .topics
                                .iter()
                                .map(|t| t.as_bytes().into())
                                .collect(),
                            data: Data::new(log_entry.data.clone()),
                            block_hash: Some(block.header.hash().as_bytes().into()),
                            block_number: Some(block.header.height.into()),
                            transaction_hash: Some(raw_receipt.transaction_hash.as_bytes().into()),
                            transaction_index: Some(tx_index.into()),
                            log_index: Some(log_index.into()),
                            transaction_log_index: Some(
                                (log_index + logs_in_block_before_receipt).into(),
                            ),
                        })
                        .collect();
                    let receipt = Receipt {
                        transaction_hash: Some(raw_receipt.transaction_hash.as_bytes().into()),
                        transaction_index: Some(tx_index.into()),
                        block_hash: Some(raw_receipt.block_hash.as_bytes().into()),
                        block_number: Some(block.header.height.into()),
                        cumulative_quota_used: 0.into(), // todo
                        quota_used: Some(raw_receipt.quota_used.into()),
                        contract_address: raw_receipt
                            .contract_address
                            .clone()
                            .map(|addr| addr.as_bytes().into()),
                        logs,
                        state_root: Some(raw_receipt.state_root.as_bytes().into()),
                        logs_bloom: raw_receipt.logs_bloom.data().clone().into(),
                        error_message: Some(raw_receipt.receipt_error.clone()),
                    };
                    ok(receipt)
                })
        });
    Box::new(fut)
}

impl<S, E, T> Chain for ChainRpcImpl<S, E, T>
where
    S: Storage + 'static,
    E: Executor + 'static,
    T: TransactionPool + 'static,
{
    fn block_number(&self) -> BoxFuture<Quantity> {
        let fut = self
            .storage
            .get_latest_block(ctx)
            .map_err(|e| {
                error!("get_latest_block err: {:?}", e);
                JsonrpcError::internal_error()
            })
            .and_then(|block| ok(Quantity::new(block.header.height.into())));

        Box::new(fut)
    }

    fn send_raw_transaction(&self, signed_data: Data) -> BoxFuture<TxResponse> {
        let transaction_pool = self.get_transaction_pool_inst();
        let fut = AsyncCodec::decode::<SerUnverifiedTransaction>(signed_data.into())
            .map_err(|e| {
                error!("decode transaction data err: {:?}", e);
                JsonrpcError::internal_error()
            })
            .map(SerUnverifiedTransaction::into)
            .and_then(move |untx| {
                transaction_pool
                    .insert(untx)
                    .map_err(|e| {
                        error!("insert transaction err: {:?}", e);
                        JsonrpcError::internal_error()
                    })
                    .map(|tx| TxResponse {
                        hash: tx.hash.into_fixed_bytes().into(),
                        status: "OK".to_string(),
                    })
            });
        Box::new(fut)
    }

    fn get_block_by_hash(&self, hash: Data32, include_tx: bool) -> BoxFuture<Option<Block>> {
        let storage = self.get_storage_inst();
        let fut = self
            .storage
            .get_block_by_hash(ctx, &transform_data32_to_hash(hash))
            .then(move |x| {
                let res: BoxFuture<_> = match x {
                    Ok(raw_block) => match raw_block {
                        Some(rblock) => Box::new(
                            get_jsonrpc_block_from_raw_block(storage, &rblock, include_tx)
                                .map(Some),
                        ),
                        None => Box::new(ok(None)),
                    },
                    Err(e) => {
                        error!("get_block_by_hash err: {:?}", e);
                        Box::new(err(JsonrpcError::internal_error()))
                    }
                };
                res
            });
        Box::new(fut)
    }

    fn get_block_by_number(&self, height: Quantity, include_tx: bool) -> BoxFuture<Option<Block>> {
        let storage = self.get_storage_inst();
        let fut = self
            .storage
            .get_block_by_height(ctx, height.into())
            .then(move |x| {
                let res: BoxFuture<_> = match x {
                    Ok(raw_block) => match raw_block {
                        Some(rblock) => Box::new(
                            get_jsonrpc_block_from_raw_block(storage, &rblock, include_tx)
                                .map(Some),
                        ),
                        None => Box::new(ok(None)),
                    },
                    Err(e) => {
                        error!("get_block_by_height err: {:?}", e);
                        Box::new(err(JsonrpcError::internal_error()))
                    }
                };
                res
            });
        Box::new(fut)
    }

    fn get_transaction_receipt(&self, hash: Data32) -> BoxFuture<Receipt> {
        let storage = self.get_storage_inst();
        let fut = self
            .storage
            .get_receipt(ctx, &transform_data32_to_hash(hash))
            .map_err(|e| {
                error!("get_receipt err: {:?}", e);
                JsonrpcError::internal_error()
            })
            .and_then(|raw_receipt| match raw_receipt {
                Some(receipt) => get_jsonrpc_receipt_from_raw_receipt(storage, receipt),
                None => {
                    error!("get_receipt err: not found");
                    Box::new(err(JsonrpcError::internal_error()))
                }
            });

        Box::new(fut)
    }

    fn get_logs(&self, filter: RpcFilter) -> BoxFuture<Vec<Log>> {
        let filter: Filter = filter.into();
        get_logs(self.get_storage_inst(), filter)
    }

    fn call(&self, call_request: CallRequest, block_number: BlockNumber) -> BoxFuture<Data> {
        let executor = self.get_executor_inst();
        let storage = self.get_storage_inst();
        let fut = get_block_by_block_number(storage, block_number.clone()).and_then(move |block| {
            let res: BoxFuture<_> = match block {
                None => Box::new(err(JsonrpcError::invalid_params_with_details(
                    format!("{:?}", &block_number),
                    "no block in the given BlockNumber",
                ))),
                Some(block) => Box::new(result(
                    executor
                        .readonly(
                            &block.header,
                            &Address::from_bytes(Into::<Vec<u8>>::into(call_request.to).as_slice())
                                .expect("never returns an error"),
                            &Address::from_bytes(
                                call_request.from.map_or(vec![], Into::into).as_slice(),
                            )
                            .expect("never returns an error"),
                            &call_request.data.map_or(vec![], Into::into),
                        )
                        .map_err(|e| {
                            error!("executor.readonly err: {:?}", e);
                            JsonrpcError::internal_error()
                        })
                        .map(|result| {
                            let vec_data = result.data.unwrap_or_else(|| vec![]);
                            vec_data.into()
                        }),
                )),
            };
            res
        });
        Box::new(fut)
    }

    fn get_transaction(&self, hash: Data32) -> BoxFuture<Option<RpcTransaction>> {
        let storage = self.get_storage_inst();
        let fut = self
            .storage
            .get_transaction(&transform_data32_to_hash(hash))
            .then(move |x| {
                let res: BoxFuture<_> = match x {
                    Ok(raw_transaction) => match raw_transaction {
                        Some(raw_tx) => {
                            Box::new(get_jsonrpc_tx_from_raw_tx(storage, raw_tx).map(Some))
                        }
                        None => Box::new(ok(None)),
                    },
                    Err(e) => {
                        error!("get_transaction err: {:?}", e);
                        Box::new(err(JsonrpcError::internal_error()))
                    }
                };
                res
            });
        Box::new(fut)
    }

    fn get_transaction_count(
        &self,
        addr: Data20,
        block_number: BlockNumber,
    ) -> BoxFuture<Quantity> {
        let storage = self.get_storage_inst();
        let fut = get_block_by_block_number(Arc::<S>::clone(&storage), block_number).and_then(
            move |block| {
                let res: BoxFuture<_> = match block {
                    Some(block) => {
                        let addr = Address::from_bytes(Into::<Vec<u8>>::into(addr).as_slice())
                            .expect("never returns an error");
                        let hashes: Vec<&Hash> = block.tx_hashes.iter().collect();
                        Box::new(
                            Arc::<S>::clone(&storage)
                                .get_transactions(ctx, &hashes)
                                .map_err(|e| {
                                    error!("get_transactions err: {:?}", e);
                                    JsonrpcError::internal_error()
                                })
                                .and_then(move |transactions| {
                                    let count = transactions
                                        .into_iter()
                                        .filter(|tx| {
                                            tx.is_some() && tx.clone().unwrap().sender == addr
                                        })
                                        .count();
                                    ok(Quantity::new(count.into()))
                                }),
                        )
                    }
                    None => Box::new(ok(Quantity::new(0.into()))),
                };
                res
            },
        );
        Box::new(fut)
    }

    fn get_code(&self, addr: Data20, block_number: BlockNumber) -> BoxFuture<Data> {
        let addr = Address::from_bytes(Into::<Vec<u8>>::into(addr).as_slice())
            .expect("never returns an error");
        let storage = self.get_storage_inst();
        let executor = self.get_executor_inst();
        let fut = get_block_by_block_number(storage, block_number).and_then(move |block| {
            let res: BoxFuture<_> = match block {
                Some(block) => Box::new(result(
                    executor
                        .get_code(&block.header.state_root, &addr)
                        .map_err(|e| {
                            error!("get_code err: {:?}", e);
                            JsonrpcError::internal_error()
                        })
                        .map(|(code, _hash)| code.into()),
                )),
                None => Box::new(err(JsonrpcError::invalid_params("block not found"))),
            };
            res
        });
        Box::new(fut)
    }

    fn get_abi(&self, _addr: Data20, _block_number: BlockNumber) -> BoxFuture<Data> {
        unimplemented!()
    }

    fn get_balance(&self, addr: Data20, block_number: BlockNumber) -> BoxFuture<Quantity> {
        let addr = Address::from_bytes(Into::<Vec<u8>>::into(addr).as_slice())
            .expect("never returns an error");
        let storage = self.get_storage_inst();
        let executor = self.get_executor_inst();
        let fut = get_block_by_block_number(storage, block_number).and_then(move |block| {
            let res: BoxFuture<_> = match block {
                Some(block) => Box::new(result(
                    executor
                        .get_balance(&block.header.state_root, &addr)
                        .map_err(|e| {
                            error!("get_balance err: {:?}", e);
                            JsonrpcError::internal_error()
                        })
                        .map(transform_balance_to_quantity),
                )),
                None => Box::new(err(JsonrpcError::invalid_params("block not found"))),
            };
            res
        });
        Box::new(fut)
    }

    fn get_transaction_proof(&self, _hash: Data32) -> BoxFuture<Data> {
        unimplemented!()
    }

    fn get_receipt_proof(&self, hash: Data32) -> BoxFuture<Option<Data>> {
        // get block body
        let storage = self.get_storage_inst();
        let storage2 = Arc::clone(&storage);
        let hash = transform_data32_to_hash(hash);

        let block_ret = get_block_by_tx_hash(storage, &hash).wait();
        let mut block: RawBlock;
        match block_ret {
            Ok(blk) => match blk {
                Some(value) => block = value,
                None => return Box::new(ok(None)),
            },
            Err(e) => {
                error!("get_block_by_tx_hash err: {:?}", e);
                return Box::new(err(JsonrpcError::internal_error()));
            }
        };
        let block_number = block.header.height;

        // get all the receipts in block
        let tx_hashes = block.tx_hashes;
        let index = tx_hashes
            .iter()
            .position(|x| x == &hash)
            .expect("tx should be in block");
        let receipts_ret = storage2
            .get_receipts(ctx, tx_hashes.iter().collect::<Vec<_>>().as_slice())
            .wait();

        let receipt_list;
        match receipts_ret {
            Ok(value) => receipt_list = value,
            Err(e) => {
                error!("get_receipts err: {:?}", e);
                return Box::new(err(JsonrpcError::internal_error()));
            }
        };

        let receipt: RawReceipt = receipt_list
            .get(index)
            .cloned()
            .expect("should get receipt if index exist")
            .unwrap();

        // get merkle proof
        let receipts: Vec<RawReceipt> = receipt_list
            .into_iter()
            .map(|r| r.expect("tx should have receipt"))
            .collect();

        let hahses: Vec<Hash> = receipts.iter().map(RawReceipt::hash).collect();
        let tree = Merkle::from_hashes(hahses.clone(), merge);
        let merkle_proof = tree
            .get_proof_by_input_index(index)
            .expect("should always return proof if index is correct");

        let receipt_proof = ReceiptProof {
            receipt,
            merkle_proof,
            block_number,
        };
        Box::new(ok(Some(Data::new(rlp::encode(&receipt_proof)))))
    }

    fn get_meta_data(&self, _block_number: BlockNumber) -> BoxFuture<MetaData> {
        unimplemented!()
    }

    fn get_block_header(&self, block_number: BlockNumber) -> BoxFuture<Data> {
        let storage = self.get_storage_inst();
        let fut = get_block_by_block_number(storage, block_number).map(|block| match block {
            Some(block) => Data::new(rlp::encode(&block.header)),
            None => Data::new(vec![]),
        });
        Box::new(fut)
    }

    fn get_state_proof(
        &self,
        _addr: Data20,
        _key: Data32,
        _block_number: BlockNumber,
    ) -> BoxFuture<Data> {
        unimplemented!()
    }

    fn get_storage_at(
        &self,
        addr: Data20,
        key: Data32,
        block_number: BlockNumber,
    ) -> BoxFuture<Data> {
        let addr = Address::from_bytes(Into::<Vec<u8>>::into(addr).as_slice())
            .expect("never returns an error");
        let storage = self.get_storage_inst();
        let executor = Arc::<E>::clone(&self.executor);
        let fut = get_block_by_block_number(storage, block_number).and_then(move |block| {
            let res: BoxFuture<_> = match block {
                Some(block) => Box::new(result(
                    executor
                        .get_value(
                            &block.header.state_root,
                            &addr,
                            &transform_data32_to_h256(key),
                        )
                        .map_err(|e| {
                            error!("get_value err: {:?}", e);
                            JsonrpcError::internal_error()
                        })
                        .map(|v| Data::new(v.to_vec())),
                )),
                None => Box::new(ok(Data::new([0; 32].to_vec()))),
            };
            res
        });
        Box::new(fut)
    }
}

fn transform_balance_to_quantity(balance: Balance) -> Quantity {
    let mut arr = [0u8; 32];
    balance.into_little_endian(&mut arr).unwrap();
    arr.as_ref().into()
}

fn transform_data32_to_h256(data: Data32) -> H256 {
    let v: Vec<u8> = data.into();
    let mut array = [0; 32];
    array.copy_from_slice(&v);
    array.into()
}

#[cfg(test)]
mod tests {
    //        use super::*;
    //        use crate::helpers::mock_storage::MockStorage;
    //        use jsonrpc_core::IoHandler;
    //
    //        fn get_io_handler() -> IoHandler {
    //            let storage = Arc::new(MockStorage::new());
    //            let mut io = IoHandler::new();
    //            let chain_rpc_impl = ChainRpcImpl::new(Arc::<MockStorage>::clone(&storage));
    //            io.extend_with(chain_rpc_impl.to_delegate());
    //            io
    //        }
    //
    //        #[test]
    //        fn test_basic() {
    //            let io = get_io_handler();
    //            let req = r#"
    //            {
    //    			"jsonrpc": "2.0",
    //    			"method": "blockNumber",
    //    			"params": [],
    //    			"id": 15
    //    		}
    //            "#;
    //            let res = io.handle_request_sync(&req).unwrap();
    //            assert_eq!(r#"{"jsonrpc":"2.0","result":"0x0","id":15}"#, &res);
    //        }
}
