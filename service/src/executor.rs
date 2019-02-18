use crate::{
    proto::{common::SignedTransaction, executor::ExecutionResp},
    Context, FutResponse,
};

pub trait ExecutorService {
    fn apply(&self, ctx: Context, tx: SignedTransaction) -> FutResponse<ExecutionResp>;
}
