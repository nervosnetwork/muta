use account::AccountService;
use asset::AssetService;
use derive_more::{Display, From};
use metadata::MetadataService;
use muta::MutaBuilder;
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

fn main() {
    let config_path =
        std::env::var("CONFIG").unwrap_or_else(|_| "devtools/chain/config.toml".to_owned());
    let genesis_path =
        std::env::var("GENESIS").unwrap_or_else(|_| "devtools/chain/genesis.toml".to_owned());

    let builder = MutaBuilder::new();

    // set configs
    let builder = builder
        .config_path(&config_path)
        .genesis_path(&genesis_path);

    // set service-mapping
    let builer = builder.service_mapping(DefaultServiceMapping {});

    let muta = builer.build().expect("build");
    muta.run().expect("run");
}

#[derive(Debug, Display, From)]
pub enum MappingError {
    #[display(fmt = "service {:?} was not found", service)]
    NotFoundService { service: String },
}
impl std::error::Error for MappingError {}

impl From<MappingError> for ProtocolError {
    fn from(err: MappingError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Service, Box::new(err))
    }
}
