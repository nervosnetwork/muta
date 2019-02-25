use core_types::{Receipt, SignedTransaction};

use crate::{Context, FutRuntimeResult};

pub trait Executor: Send + Sync {
    type Error;

    fn call(
        &mut self,
        ctx: &Context,
        tx: SignedTransaction,
    ) -> FutRuntimeResult<Receipt, Self::Error>;
    fn static_call(
        &self,
        ctx: &Context,
        tx: SignedTransaction,
    ) -> FutRuntimeResult<Receipt, Self::Error>;
}
