use std::marker::{Send, Sync};

use grpc::{RequestOptions, SingleResponse};

use crate::{
    grpc::GrpcExecutorService,
    proto::{blockchain::SignedTransaction, executor::ExecutionResp},
    service::{Context, ExecutorService},
    ContextExchange, FutResponseExt,
};

pub struct ExecutorServer {
    server: ::grpc::Server,
}

impl_server!(ExecutorServer, GrpcExecutorImpl, EXECUTOR);

struct GrpcExecutorImpl<T> {
    core_srv: T,
}

impl<T: ExecutorService + Sync + Send + 'static> GrpcExecutorService for GrpcExecutorImpl<T> {
    fn apply(&self, o: RequestOptions, tx: SignedTransaction) -> SingleResponse<ExecutionResp> {
        self.core_srv
            .apply(Context::from_rpc_context(o), tx)
            .into_single_resp()
    }
}
