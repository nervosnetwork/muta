#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use binding_macro::{cycles, genesis, service};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK, ServiceSchema};
use protocol::types::{Metadata, ServiceContext, METADATA_KEY};

pub struct MetadataService<SDK> {
    sdk: SDK,
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

    #[cycles(210_00)]
    #[read]
    fn get_metadata(&self, ctx: ServiceContext) -> ServiceResponse<Metadata> {
        let metadata: Metadata = self
            .sdk
            .get_value(&METADATA_KEY.to_owned())
            .expect("metadata should not be none");
        ServiceResponse::<Metadata>::from_succeed(metadata)
    }
}
