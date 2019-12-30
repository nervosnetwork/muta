use derive_more::{Display, From};

use asset::AssetService;
use muta::MutaBuilder;
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
            "asset" => Box::new(AssetService::init(sdk)?) as Box<dyn Service>,
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
        vec!["asset".to_owned()]
    }
}

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
    let builder = MutaBuilder::new();

    // set configs
    let builder = builder
        .config_path("devtools/chain/config.toml")
        .genesis_path("devtools/chain/genesis.toml");

    // set service-mapping
    let builer = builder.service_mapping(DefaultServiceMapping {});

    let muta = builer.build().unwrap();

    muta.run().await.unwrap()
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
