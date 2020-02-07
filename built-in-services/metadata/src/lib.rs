#[cfg(test)]
mod tests;
mod types;

use derive_more::{Display, From};

use binding_macro::{cycles, genesis, service};
use protocol::traits::{ExecutorParams, ServiceSDK};
use protocol::types::{Address, Metadata, ServiceContext, METADATA_KEY};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::types::{
    SetAdminPayload, UpdateIntervalPayload, UpdateMetadataPayload, UpdateRatioPayload,
    UpdateValidatorsPayload,
};

pub const ADMIN_KEY: &str = "admin";
pub const ADMIN_GENESIS: &str = "0xf8389d774afdad8755ef8e629e5a154fddc6325a";

pub struct MetadataService<SDK> {
    sdk: SDK,
}

#[service]
impl<SDK: ServiceSDK> MetadataService<SDK> {
    pub fn new(sdk: SDK) -> ProtocolResult<Self> {
        Ok(Self { sdk })
    }

    #[genesis]
    fn init_genesis(&mut self, metadata: Metadata) -> ProtocolResult<()> {
        self.sdk.set_value(METADATA_KEY.to_string(), metadata)?;

        let admin = Address::from_hex(ADMIN_GENESIS)?;
        self.sdk.set_value(ADMIN_KEY.to_string(), admin)
    }

    #[cycles(210_00)]
    #[read]
    fn get_metadata(&self, ctx: ServiceContext) -> ProtocolResult<Metadata> {
        let metadata: Metadata = self
            .sdk
            .get_value(&METADATA_KEY.to_owned())?
            .expect("Metadata should always be in the genesis block");
        Ok(metadata)
    }

    #[cycles(210_00)]
    #[read]
    fn get_admin(&self, ctx: ServiceContext) -> ProtocolResult<Address> {
        let admin: Address = self
            .sdk
            .get_value(&ADMIN_KEY.to_owned())?
            .expect("Admin should not be none");
        Ok(admin)
    }

    #[cycles(210_00)]
    #[write]
    fn update_metadata(
        &mut self,
        ctx: ServiceContext,
        payload: UpdateMetadataPayload,
    ) -> ProtocolResult<()> {
        if self.verify_authority(ctx.get_caller())? {
            let mut metadata: Metadata = self
                .sdk
                .get_value(&METADATA_KEY.to_owned())?
                .expect("Metadata should always be in the genesis block");

            metadata.verifier_list = payload.verifier_list;
            metadata.interval = payload.interval;
            metadata.precommit_ratio = payload.precommit_ratio;
            metadata.prevote_ratio = payload.prevote_ratio;
            metadata.propose_ratio = payload.propose_ratio;

            self.sdk
                .set_value(METADATA_KEY.to_string(), metadata.clone())?;
            let event_str = serde_json::to_string(&metadata).map_err(ServiceError::JsonParse)?;
            ctx.emit_event(event_str)
        } else {
            Err(ServiceError::NonAuthorized.into())
        }
    }

    #[cycles(210_00)]
    #[write]
    fn update_validators(
        &mut self,
        ctx: ServiceContext,
        payload: UpdateValidatorsPayload,
    ) -> ProtocolResult<()> {
        if self.verify_authority(ctx.get_caller())? {
            let mut metadata: Metadata = self
                .sdk
                .get_value(&METADATA_KEY.to_owned())?
                .expect("Metadata should always be in the genesis block");

            metadata.verifier_list = payload.verifier_list.clone();

            self.sdk.set_value(METADATA_KEY.to_string(), metadata)?;
            let event_str = serde_json::to_string(&payload).map_err(ServiceError::JsonParse)?;
            ctx.emit_event(event_str)
        } else {
            Err(ServiceError::NonAuthorized.into())
        }
    }

    #[cycles(210_00)]
    #[write]
    fn update_interval(
        &mut self,
        ctx: ServiceContext,
        payload: UpdateIntervalPayload,
    ) -> ProtocolResult<()> {
        if self.verify_authority(ctx.get_caller())? {
            let mut metadata: Metadata = self
                .sdk
                .get_value(&METADATA_KEY.to_owned())?
                .expect("Metadata should always be in the genesis block");
            metadata.interval = payload.interval;
            self.sdk.set_value(METADATA_KEY.to_string(), metadata)?;
            let event_str = serde_json::to_string(&payload).map_err(ServiceError::JsonParse)?;
            ctx.emit_event(event_str)
        } else {
            Err(ServiceError::NonAuthorized.into())
        }
    }

    #[cycles(210_00)]
    #[write]
    fn update_ratio(
        &mut self,
        ctx: ServiceContext,
        payload: UpdateRatioPayload,
    ) -> ProtocolResult<()> {
        if self.verify_authority(ctx.get_caller())? {
            let mut metadata: Metadata = self
                .sdk
                .get_value(&METADATA_KEY.to_owned())?
                .expect("Metadata should always be in the genesis block");

            metadata.precommit_ratio = payload.precommit_ratio;
            metadata.prevote_ratio = payload.prevote_ratio;
            metadata.propose_ratio = payload.propose_ratio;

            self.sdk.set_value(METADATA_KEY.to_string(), metadata)?;
            let event_str = serde_json::to_string(&payload).map_err(ServiceError::JsonParse)?;
            ctx.emit_event(event_str)
        } else {
            Err(ServiceError::NonAuthorized.into())
        }
    }

    #[cycles(210_00)]
    #[write]
    fn set_admin(&mut self, ctx: ServiceContext, payload: SetAdminPayload) -> ProtocolResult<()> {
        if self.verify_authority(ctx.get_caller())? {
            self.sdk
                .set_value(ADMIN_KEY.to_owned(), payload.admin.clone())?;
            let event_str = serde_json::to_string(&payload).map_err(ServiceError::JsonParse)?;
            ctx.emit_event(event_str)
        } else {
            Err(ServiceError::NonAuthorized.into())
        }
    }

    fn verify_authority(&self, caller: Address) -> ProtocolResult<bool> {
        let admin: Address = self
            .sdk
            .get_value(&ADMIN_KEY.to_string())?
            .expect("Admin should not be none");

        if caller == admin {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Debug, Display, From)]
pub enum ServiceError {
    NonAuthorized,

    #[display(fmt = "Parsing payload to json failed {:?}", _0)]
    JsonParse(serde_json::Error),
}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for ProtocolError {
    fn from(err: ServiceError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
