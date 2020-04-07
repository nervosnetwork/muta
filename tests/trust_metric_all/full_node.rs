mod builder;
mod config;
mod default_start;
mod error;

use builder::MutaBuilder;

use asset::AssetService;
use derive_more::{Display, From};
use metadata::MetadataService;
use protocol::traits::{Service, ServiceMapping, ServiceSDK};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

struct DefaultServiceMapping;

impl ServiceMapping for DefaultServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK>(
        &self,
        name: &str,
        sdk: SDK,
    ) -> ProtocolResult<Box<dyn Service>> {
        let service = match name {
            "asset" => Box::new(AssetService::new(sdk)) as Box<dyn Service>,
            "metadata" => Box::new(MetadataService::new(sdk)) as Box<dyn Service>,
            _ => {
                return Err(MappingError::NotFoundService {
                    service: name.to_owned(),
                }
                .into())
            }
        };

        Ok(service)
    }

    fn list_service_name(&self) -> Vec<String> {
        vec!["asset".to_owned(), "metadata".to_owned()]
    }
}

#[derive(Debug, Display, From)]
enum MappingError {
    #[display(fmt = "service {:?} was not found", service)]
    NotFoundService { service: String },
}

impl std::error::Error for MappingError {}

impl From<MappingError> for ProtocolError {
    fn from(err: MappingError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}

pub fn run() {
    let builder = MutaBuilder::new()
        .config_path("tests/trust_metric_all/config/config.toml")
        .genesis_path("tests/trust_metric_all/config/genesis.toml")
        .service_mapping(DefaultServiceMapping {});

    let muta = builder.build().expect("build");
    muta.run().expect("run");
}
