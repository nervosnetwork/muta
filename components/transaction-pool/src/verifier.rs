use core_runtime::{pool::Verifier, Context, FutRuntimeResult};
use core_types::transaction::{SignedTransaction, UnverifiedTransaction};

use crate::errors::VerifierError;

#[derive(Debug)]
pub struct SECP256K1Verifier {}

impl SECP256K1Verifier {
    pub fn new() -> Self {
        SECP256K1Verifier {}
    }
}

impl Verifier for SECP256K1Verifier {
    type Error = VerifierError;

    fn unverified_transaction(
        &self,
        ctx: &Context,
        untx: UnverifiedTransaction,
    ) -> FutRuntimeResult<SignedTransaction, Self::Error> {
        unimplemented!()
    }
}
