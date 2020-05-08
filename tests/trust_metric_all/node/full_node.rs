mod builder;
mod default_start;
mod error;
mod memory_db;

use super::{common, config, consts, diagnostic};
use builder::MutaBuilder;

use account::AccountService;
use asset::AssetService;
use derive_more::{Display, From};
use metadata::MetadataService;
use protocol::traits::{Service, ServiceMapping, ServiceSDK};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};
use util::UtilService;

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
            "util" => Box::new(UtilService::new(sdk)) as Box<dyn Service>,
            "account" => Box::new(AccountService::new(sdk)) as Box<dyn Service>,
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
        vec![
            "asset".to_owned(),
            "metadata".to_owned(),
            "util".to_owned(),
            "account".to_owned(),
        ]
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

pub async fn run(listen_port: u16) {
    let builder = MutaBuilder::new()
        .config_path(consts::CHAIN_CONFIG_PATH)
        .genesis_path(consts::CHAIN_GENESIS_PATH)
        .service_mapping(DefaultServiceMapping {});

    let muta = builder.build(listen_port).expect("build");
    muta.run().await.expect("run");
}
