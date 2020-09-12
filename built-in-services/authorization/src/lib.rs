use binding_macro::{cycles, service};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{ServiceContext, SignedTransaction};
use serde::Deserialize;

use multi_signature::MultiSignatureService;

pub const AUTHORIZATION_SERVICE_NAME: &str = "authorization";

#[derive(Deserialize)]
pub struct PtrSignedTransaction {
    ptr: usize,
}

pub struct AuthorizationService<SDK> {
    _sdk:      SDK,
    multi_sig: MultiSignatureService<SDK>,
}

#[service]
impl<SDK: ServiceSDK> AuthorizationService<SDK> {
    pub fn new(_sdk: SDK, multi_sig: MultiSignatureService<SDK>) -> Self {
        Self { _sdk, multi_sig }
    }

    #[cycles(21_000)]
    #[read]
    fn check_authorization_by_ptr(
        &self,
        ctx: ServiceContext,
        payload: PtrSignedTransaction,
    ) -> ServiceResponse<()> {
        let stx: SignedTransaction = {
            let boxed = unsafe { Box::from_raw(payload.ptr as *mut SignedTransaction) };
            *boxed
        };

        self.check_authorization(ctx, stx)
    }

    #[cycles(21_000)]
    #[read]
    fn check_authorization(
        &self,
        ctx: ServiceContext,
        payload: SignedTransaction,
    ) -> ServiceResponse<()> {
        let resp = self.multi_sig.verify_signature(ctx, payload);
        if resp.is_error() {
            return ServiceResponse::<()>::from_error(
                102,
                format!(
                    "verify transaction signature error {:?}",
                    resp.error_message
                ),
            );
        }

        ServiceResponse::from_succeed(())
    }
}
