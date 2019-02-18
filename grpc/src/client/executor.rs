use crate::{
    grpc::{GrpcExecutorClient, GrpcExecutorService},
    proto::{blockchain::SignedTransaction, executor::ExecutionResp},
    service::{Context, ExecutorService, FutResponse},
    ContextExchange, SingleResponseExt,
};

pub struct ExecutorClient {
    client: GrpcExecutorClient,
}

impl_client!(ExecutorClient, EXECUTOR_CLIENT_HOST, EXECUTOR_CLIENT_PORT);

impl ExecutorService for ExecutorClient {
    fn apply(&self, ctx: Context, tx: SignedTransaction) -> FutResponse<ExecutionResp> {
        self.client
            .apply(ctx.into_rpc_context(), tx)
            .into_fut_resp()
    }
}
