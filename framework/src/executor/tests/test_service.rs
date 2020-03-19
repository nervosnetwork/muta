use serde::{Deserialize, Serialize};

use binding_macro::{cycles, service, tx_hook_after, tx_hook_before};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::ServiceContext;

pub struct TestService<SDK> {
    sdk: SDK,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TestReadPayload {
    pub key: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct TestReadResponse {
    pub value: String,
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

    #[cycles(100_00)]
    #[read]
    fn test_read(
        &self,
        ctx: ServiceContext,
        payload: TestReadPayload,
    ) -> ServiceResponse<TestReadResponse> {
        let value: String = self.sdk.get_value(&payload.key).unwrap_or_default();
        let res = TestReadResponse { value };
        ServiceResponse::<TestReadResponse>::from_data(res)
    }

    #[cycles(210_00)]
    #[write]
    fn test_write(
        &mut self,
        ctx: ServiceContext,
        payload: TestWritePayload,
    ) -> ServiceResponse<TestWriteResponse> {
        self.sdk.set_value(payload.key, payload.value);
        ServiceResponse::<TestWriteResponse>::from_data(TestWriteResponse {})
    }

    #[cycles(210_00)]
    #[write]
    fn test_service_call_invoke_hook_only_once(
        &mut self,
        ctx: ServiceContext,
        payload: TestWritePayload,
    ) -> ServiceResponse<TestWriteResponse> {
        let payload_str = serde_json::to_string(&payload).unwrap();
        self.sdk
            .write(&ctx, None, "test", "test_write", &payload_str);
        ServiceResponse::<TestWriteResponse>::from_data(TestWriteResponse {})
    }

    #[tx_hook_before]
    fn test_tx_hook_before(&mut self, ctx: ServiceContext) {
        if ctx.get_service_name() == "test"
            && ctx.get_payload().to_owned().contains("test_hook_before")
        {
            ctx.emit_event("test_tx_hook_before invoked".to_owned());
        }
    }

    #[tx_hook_after]
    fn test_tx_hook_after(&mut self, ctx: ServiceContext) {
        if ctx.get_service_name() == "test"
            && ctx.get_payload().to_owned().contains("test_hook_after")
        {
            ctx.emit_event("test_tx_hook_after invoked".to_owned());
        }
    }
}
