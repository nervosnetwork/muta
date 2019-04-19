use crate::helpers::{get_current_height, get_logs, FilterManager, FilterType};
use crate::types::Filter;
use core_storage::storage::Storage;
use futures::future::{join_all, ok, result, Future};
use jsonrpc_core::{BoxFuture, Error as JsonrpcError};
use jsonrpc_derive::rpc;
use jsonrpc_types::rpctypes::{
    BlockNumber, Data32, Filter as RpcFilter, FilterChanges, Log, Quantity,
};
use log::error;
use parking_lot::Mutex;
use std::sync::Arc;

#[rpc]
pub trait ChainFilter {
    #[rpc(name = "newFilter")]
    fn new_filter(&self, filter: RpcFilter) -> BoxFuture<Quantity>;

    #[rpc(name = "newBlockFilter")]
    fn new_block_filter(&self) -> BoxFuture<Quantity>;

    #[rpc(name = "uninstallFilter")]
    fn uninstall_filter(&self, filter_id: Quantity) -> BoxFuture<bool>;

    #[rpc(name = "getFilterChanges")]
    fn get_filter_changes(&self, filter_id: Quantity) -> BoxFuture<FilterChanges>;

    #[rpc(name = "getFilterLogs")]
    fn get_filter_logs(&self, filter_id: Quantity) -> BoxFuture<Vec<Log>>;
}

pub struct ChainFilterRpcImpl<S>
where
    S: Storage,
{
    storage: Arc<S>,
    filter_manager: Arc<Mutex<FilterManager<FilterType>>>,
}

impl<S> ChainFilterRpcImpl<S>
where
    S: Storage + 'static,
{
    pub fn new(storage: Arc<S>) -> Self {
        ChainFilterRpcImpl {
            storage,
            filter_manager: Arc::new(Mutex::new(FilterManager::default())),
        }
    }

    pub fn get_storage_inst(&self) -> Arc<S> {
        Arc::<S>::clone(&self.storage)
    }

    pub fn get_filter_manager(&self) -> Arc<Mutex<FilterManager<FilterType>>> {
        Arc::clone(&self.filter_manager)
    }
}

impl<S> ChainFilter for ChainFilterRpcImpl<S>
where
    S: Storage + 'static,
{
    fn new_filter(&self, filter: RpcFilter) -> BoxFuture<Quantity> {
        let filter_manager = self.get_filter_manager();
        let storage = self.get_storage_inst();
        let fut = get_current_height(storage)
            .map(move |height| {
                filter_manager
                    .lock()
                    .new_filter(FilterType::Logs(height.into(), filter))
            })
            .map(|id| (id as u64).into());

        Box::new(fut)
    }

    fn new_block_filter(&self) -> BoxFuture<Quantity> {
        let filter_manager = self.get_filter_manager();
        let storage = self.get_storage_inst();
        let fut = get_current_height(storage)
            .map(move |height| {
                filter_manager
                    .lock()
                    .new_filter(FilterType::Block(height.into()))
            })
            .map(|id| (id as u64).into());
        Box::new(fut)
    }

    fn uninstall_filter(&self, filter_id: Quantity) -> BoxFuture<bool> {
        let index = Into::<u64>::into(filter_id) as usize;
        let filter_manager = self.get_filter_manager();
        let mut filter_manager = filter_manager.lock();
        let is_uninstall = match filter_manager.get(index) {
            Some(_) => {
                filter_manager.uninstall_filter(index);
                true
            }
            None => false,
        };
        Box::new(ok(is_uninstall))
    }

    fn get_filter_changes(&self, filter_id: Quantity) -> BoxFuture<FilterChanges> {
        let index = Into::<u64>::into(filter_id) as usize;
        let storage = self.get_storage_inst();
        let filter_manager = self.get_filter_manager();
        let res = match filter_manager.lock().get_mut(index) {
            None => Ok(FilterChanges::Empty),
            Some(filter) => match *filter {
                FilterType::Block(ref mut block_number) => {
                    get_current_height(Arc::<S>::clone(&storage))
                        .map(Into::<u64>::into)
                        .and_then(move |height| {
                            let current_number = height + 1;
                            let hashes_fut =
                                join_all(((*block_number + 1)..current_number).map(move |h| {
                                    Arc::<S>::clone(&storage)
                                        .get_block_by_height(ctx, h)
                                        .map_err(|e| {
                                            error!("get_block_by_height err: {:?}", e);
                                            JsonrpcError::internal_error()
                                        })
                                        .map(|blk| match blk {
                                            Some(block) => {
                                                Data32::new(block.header.hash().as_bytes().into())
                                            }
                                            None => {
                                                error!("get_block_by_height err: Not found");
                                                // TODO: please fix it!!!
                                                Data32::default()
                                            }
                                        })
                                }))
                                .map(FilterChanges::Hashes);
                            *block_number = current_number;
                            hashes_fut
                        })
                        .wait() // todo: remove wait here
                }
                FilterType::Logs(ref mut block_number, ref filter) => {
                    get_current_height(Arc::<S>::clone(&storage))
                        .map(Into::<u64>::into)
                        .and_then(move |current_number| {
                            let mut filter: Filter = filter.clone().into();
                            filter.from_block = BlockNumber::Height((*block_number).into());
                            filter.to_block = BlockNumber::Height(current_number.into());
                            *block_number = current_number + 1;
                            get_logs(Arc::<S>::clone(&storage), filter).map(FilterChanges::Logs)
                        })
                        .wait() // todo: remove wait here
                }
            },
        };
        Box::new(result(res))
    }

    fn get_filter_logs(&self, filter_id: Quantity) -> BoxFuture<Vec<Log>> {
        let index = Into::<u64>::into(filter_id) as usize;
        let filter_manager = self.get_filter_manager();

        let res = match filter_manager.lock().get(index) {
            Some(&FilterType::Logs(ref _block_number, ref filter)) => {
                let filter: Filter = filter.clone().into();
                get_logs(self.get_storage_inst(), filter).wait() // todo: remove wait here
            }
            _ => Ok(vec![]),
        };
        Box::new(result(res))
    }
}
