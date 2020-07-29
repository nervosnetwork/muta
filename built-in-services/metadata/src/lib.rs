#[cfg(test)]
mod tests;

use binding_macro::{cycles, genesis, service};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{Metadata, ServiceContext, METADATA_KEY};

macro_rules! impl_meatdata {
    ($self: expr, $method: ident, $ctx: expr) => {{
        let res = $self.$method($ctx.clone());
        if res.is_error() {
            Err(ServiceResponse::from_error(res.code, res.error_message))
        } else {
            Ok(res.succeed_data)
        }
    }};
    ($self: expr, $method: ident, $ctx: expr, $payload: expr) => {{
        let res = $self.$method($ctx.clone(), $payload);
        if res.is_error() {
            Err(ServiceResponse::from_error(res.code, res.error_message))
        } else {
            Ok(res.succeed_data)
        }
    }};
}

pub trait MetaData {
    fn get_(&self, ctx: &ServiceContext) -> Result<Metadata, ServiceResponse<()>>;
}

pub struct MetadataService<SDK> {
    sdk: SDK,
}

impl<SDK: ServiceSDK> MetaData for MetadataService<SDK> {
    fn get_(&self, ctx: &ServiceContext) -> Result<Metadata, ServiceResponse<()>> {
        impl_meatdata!(self, get_metadata, ctx)
    }
}

#[service]
impl<SDK: ServiceSDK> MetadataService<SDK> {
    pub fn new(sdk: SDK) -> Self {
        Self { sdk }
    }

    #[genesis]
    fn init_genesis(&mut self, metadata: Metadata) {
        self.sdk.set_value(METADATA_KEY.to_string(), metadata)
    }

    #[cycles(21_000)]
    #[read]
    fn get_metadata(&self, ctx: ServiceContext) -> ServiceResponse<Metadata> {
        let metadata: Metadata = self
            .sdk
            .get_value(&METADATA_KEY.to_owned())
            .expect("metadata should not be none");
        ServiceResponse::<Metadata>::from_succeed(metadata)
    }
}
