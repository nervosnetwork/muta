use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use binding_macro::{cycles, service, tx_hook_after, tx_hook_before, SchemaObject};
use protocol::traits::{ExecutorParams, MetaGenerator, ServiceResponse, ServiceSDK};
use protocol::types::{DataMeta, FieldMeta, MethodMeta, ServiceContext, ServiceMeta, StructMeta};

pub struct TestService<SDK> {
    sdk: SDK,
}

#[derive(Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct TestReadPayload {
    pub key: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, SchemaObject)]
pub struct TestReadResponse {
    pub value: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, SchemaObject)]
pub struct TestWritePayload {
    pub key:   String,
    pub value: String,
    pub extra: String,
}

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
        ServiceResponse::<TestReadResponse>::from_succeed(res)
    }

    #[cycles(210_00)]
    #[write]
    fn test_write(
        &mut self,
        ctx: ServiceContext,
        payload: TestWritePayload,
    ) -> ServiceResponse<()> {
        self.sdk.set_value(payload.key, payload.value);
        ServiceResponse::<()>::from_succeed(())
    }

    #[cycles(210_00)]
    #[write]
    fn test_service_call_invoke_hook_only_once(
        &mut self,
        ctx: ServiceContext,
        payload: TestWritePayload,
    ) -> ServiceResponse<()> {
        let payload_str = serde_json::to_string(&payload).unwrap();
        self.sdk
            .write(&ctx, None, "test", "test_write", &payload_str);
        ServiceResponse::<()>::from_succeed(())
    }

    #[tx_hook_before]
    fn test_tx_hook_before(&mut self, ctx: ServiceContext) {
        if ctx.get_service_name() == "test"
            && ctx.get_payload().to_owned().contains("test_hook_before")
        {
            ctx.emit_event(
                "hook before".to_owned(),
                "test_tx_hook_before invoked".to_owned(),
            );
        }
    }

    #[tx_hook_after]
    fn test_tx_hook_after(&mut self, ctx: ServiceContext) {
        if ctx.get_service_name() == "test"
            && ctx.get_payload().to_owned().contains("test_hook_after")
        {
            ctx.emit_event(
                "hook after".to_owned(),
                "test_tx_hook_after invoked".to_owned(),
            );
        }
    }
}
