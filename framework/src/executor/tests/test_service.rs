use derive_more::{Display, From};
use serde::{Deserialize, Serialize};

use binding_macro::{cycles, service, tx_hook_after, tx_hook_before};
use protocol::traits::{ExecutorParams, ServiceSDK};
use protocol::types::ServiceContext;
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

pub struct TestService<SDK> {
    sdk: SDK,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TestReadPayload {
    pub key: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TestReadResponse {
    pub value: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TestWritePayload {
    pub key:   String,
    pub value: String,
    pub extra: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TestWriteResponse {}

#[service]
impl<SDK: ServiceSDK> TestService<SDK> {
    pub fn new(sdk: SDK) -> ProtocolResult<Self> {
        Ok(Self { sdk })
    }

    #[cycles(100_00)]
    #[read]
    fn test_read(
        &self,
        ctx: ServiceContext,
        payload: TestReadPayload,
    ) -> ProtocolResult<TestReadResponse> {
        let value: String = self.sdk.get_value(&payload.key)?.unwrap_or_default();
        Ok(TestReadResponse { value })
    }

    #[cycles(210_00)]
    #[write]
    fn test_write(
        &mut self,
        ctx: ServiceContext,
        payload: TestWritePayload,
    ) -> ProtocolResult<TestWriteResponse> {
        self.sdk.set_value(payload.key, payload.value)?;
        Ok(TestWriteResponse {})
    }

    #[cycles(210_00)]
    #[write]
    fn test_service_call_invoke_hook_only_once(
        &mut self,
        ctx: ServiceContext,
        payload: TestWritePayload,
    ) -> ProtocolResult<TestWriteResponse> {
        let payload_str = serde_json::to_string(&payload).unwrap();
        self.sdk
            .write(&ctx, None, "test", "test_write", &payload_str)?;
        Ok(TestWriteResponse {})
    }

    #[tx_hook_before]
    fn test_tx_hook_before(&mut self, ctx: ServiceContext) -> ProtocolResult<()> {
        if ctx.get_service_name() == "test"
            && ctx.get_payload().to_owned().contains("test_hook_before")
        {
            ctx.emit_event("test_tx_hook_before invoked".to_owned())?;
        }
        Ok(())
    }

    #[tx_hook_after]
    fn test_tx_hook_after(&mut self, ctx: ServiceContext) -> ProtocolResult<()> {
        if ctx.get_service_name() == "test"
            && ctx.get_payload().to_owned().contains("test_hook_after")
        {
            ctx.emit_event("test_tx_hook_after invoked".to_owned())?;
        }
        Ok(())
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceError {}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for ProtocolError {
    fn from(err: ServiceError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
