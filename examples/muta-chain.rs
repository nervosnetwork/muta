use derive_more::{Display, From};

use asset::AssetService;
use muta::MutaBuilder;
use protocol::traits::{Service, ServiceMapping, ServiceSDK};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};
use riscv::RiscvService;

struct DefaultServiceMapping;

impl ServiceMapping for DefaultServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK>(
        &self,
        name: &str,
        sdk: SDK,
    ) -> ProtocolResult<Box<dyn Service>> {
        let service = match name {
            "riscv" => Box::new(RiscvService::init(sdk)?) as Box<dyn Service>,
            "asset" => Box::new(AssetService::new(sdk)?) as Box<dyn Service>,
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
        vec!["asset".to_owned(), "riscv".to_owned()]
    }
}

#[runtime::main(runtime_tokio::Tokio)]
async fn main() {
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
