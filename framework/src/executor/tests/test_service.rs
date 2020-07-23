use serde::{Deserialize, Serialize};

use binding_macro::{cycles, service, tx_hook_after, tx_hook_before};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::ServiceContext;

pub struct TestService<SDK> {
    sdk: SDK,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TestWritePayload {
    pub key:   String,
    pub value: String,
    pub extra: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct TestWriteResponse {}

#[service]
impl<SDK: ServiceSDK> TestService<SDK> {
    pub fn new(sdk: SDK) -> Self {
        Self { sdk }
    }

    #[cycles(10_000)]
    #[read]
    fn test_read(&self, ctx: ServiceContext, payload: String) -> ServiceResponse<String> {
        let value: String = self.sdk.get_value(&payload).unwrap_or_default();
        ServiceResponse::from_succeed(value)
    }

    #[cycles(21_000)]
    #[write]
    fn test_write(
        &mut self,
        ctx: ServiceContext,
        payload: TestWritePayload,
    ) -> ServiceResponse<TestWriteResponse> {
        self.sdk.set_value(payload.key, payload.value);
        ServiceResponse::<TestWriteResponse>::from_succeed(TestWriteResponse {})
    }

    #[cycles(21_000)]
    #[write]
    fn test_revert_event(
        &mut self,
        ctx: ServiceContext,
        _: TestWritePayload,
    ) -> ServiceResponse<TestWriteResponse> {
        ServiceResponse::from_error(111, "error".to_owned())
    }

    #[cycles(21_000)]
    #[write]
    fn test_event(
        &mut self,
        ctx: ServiceContext,
        _: TestWritePayload,
    ) -> ServiceResponse<TestWriteResponse> {
        ctx.emit_event("test-name".to_owned(), "test".to_owned());
        ServiceResponse::from_succeed(TestWriteResponse::default())
    }

    #[cycles(21_000)]
    #[write]
    fn test_service_call_invoke_hook_only_once(
        &mut self,
        ctx: ServiceContext,
        payload: TestWritePayload,
    ) -> ServiceResponse<TestWriteResponse> {
        self.test_write(ctx, payload);
        ServiceResponse::<TestWriteResponse>::from_succeed(TestWriteResponse {})
    }

    #[cycles(21_000)]
    #[write]
    fn test_panic(&mut self, ctx: ServiceContext, _payload: String) -> ServiceResponse<()> {
        panic!("hello panic");
    }

    #[cycles(21_000)]
    #[write]
    fn tx_hook_before_panic(
        &mut self,
        ctx: ServiceContext,
        _payload: String,
    ) -> ServiceResponse<()> {
        self.sdk.set_value(
            "tx_hook_before_panic".to_owned(),
            "tx_hook_before_panic".to_owned(),
        );
        ServiceResponse::from_succeed(())
    }

    #[cycles(21_000)]
    #[write]
    fn tx_hook_after_panic(
        &mut self,
        ctx: ServiceContext,
        _payload: String,
    ) -> ServiceResponse<()> {
        self.sdk.set_value(
            "tx_hook_after_panic".to_owned(),
            "tx_hook_after_panic".to_owned(),
        );
        ServiceResponse::from_succeed(())
    }

    #[tx_hook_before]
    fn test_tx_hook_before(&mut self, ctx: ServiceContext) -> ServiceResponse<()> {
        if ctx.get_service_name() == "test"
            && ctx.get_payload().to_owned().contains("test_hook_before")
        {
            ctx.emit_event(
                "test-name".to_owned(),
                "test_tx_hook_before invoked".to_owned(),
            );
        }

        if ctx.get_service_method() == "tx_hook_before_panic" {
            panic!("tx hook before");
        }

        self.sdk.set_value("before".to_owned(), "before".to_owned());
        ServiceResponse::from_succeed(())
    }

    #[tx_hook_after]
    fn test_tx_hook_after(&mut self, ctx: ServiceContext) -> ServiceResponse<()> {
        if ctx.get_service_name() == "test"
            && ctx.get_payload().to_owned().contains("test_hook_after")
        {
            ctx.emit_event(
                "test-name".to_owned(),
                "test_tx_hook_after invoked".to_owned(),
            );
        }

        if ctx.get_service_method() == "tx_hook_after_panic" {
            panic!("tx hook before");
        }

        self.sdk.set_value("after".to_owned(), "after".to_owned());
        ServiceResponse::from_succeed(())
    }
}
