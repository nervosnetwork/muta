#[cfg(test)]
mod tests;

use binding_macro::{cycles, genesis, service};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{Metadata, ServiceContext, METADATA_KEY};

pub const METADATA_SERVICE_NAME: &str = "metadata";

pub trait MetaData {
    fn get_(&self, ctx: &ServiceContext) -> ServiceResponse<Metadata>;
}

pub struct MetadataService<SDK> {
    sdk: SDK,
}

impl<SDK: ServiceSDK> MetaData for MetadataService<SDK> {
    fn get_(&self, ctx: &ServiceContext) -> ServiceResponse<Metadata> {
        self.get_metadata(ctx.clone())
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
