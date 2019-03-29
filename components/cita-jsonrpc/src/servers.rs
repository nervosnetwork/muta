use crate::config::Config;
use crate::module::{Chain, ChainFilter, ChainFilterRpcImpl, ChainRpcImpl, Net, NetRpcImpl};
use core_runtime::{Executor, TransactionPool};
use core_storage::storage::Storage;
use jsonrpc_core::IoHandler;
use jsonrpc_http_server::{Server, ServerBuilder};
use std::io;
use std::sync::Arc;

pub struct RpcServer {
    server: Server,
}

impl RpcServer {
    pub fn new<S, E, T>(
        config: Config,
        storage: Arc<S>,
        executor: Arc<E>,
        transaction_pool: Arc<T>,
    ) -> io::Result<Self>
    where
        S: Storage + 'static,
        E: Executor + 'static,
        T: TransactionPool + 'static,
    {
        let mut io = IoHandler::new();

        io.extend_with(NetRpcImpl.to_delegate());

        let chain_rpc_impl = ChainRpcImpl::new(
            Arc::<S>::clone(&storage),
            Arc::<E>::clone(&executor),
            Arc::<T>::clone(&transaction_pool),
        );
        io.extend_with(chain_rpc_impl.to_delegate());

        let chain_filter_rpc_impl = ChainFilterRpcImpl::new(Arc::<S>::clone(&storage));
        io.extend_with(chain_filter_rpc_impl.to_delegate());

        let server = ServerBuilder::new(io)
            //            .cors(DomainsValidation::AllowOnly(vec![
            //                AccessControlAllowOrigin::Null,
            //                AccessControlAllowOrigin::Any,
            //            ]))
            .threads(config.threads)
            .max_request_body_size(config.max_request_body_size)
            .start_http(&config.listen_address.parse().unwrap())?;

        log::info!("Jsonrpc service listening on {:?}", server.address());
        Ok(Self { server })
    }

    pub fn wait(self) {
        self.server.wait()
    }

    pub fn close(self) {
        self.server.close()
    }
}
