mod builder;
mod default_start;
mod error;
mod memory_db;

use super::{config, consts, diagnostic, sync::Sync};
use builder::MutaBuilder;

use asset::AssetService;
use authorization::AuthorizationService;
use derive_more::{Display, From};
use metadata::MetadataService;
use multi_signature::MultiSignatureService;
use protocol::traits::{SDKFactory, Service, ServiceMapping, ServiceSDK};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

struct DefaultServiceMapping;

impl ServiceMapping for DefaultServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK, Factory: SDKFactory<SDK>>(
        &self,
        name: &str,
        factory: &Factory,
    ) -> ProtocolResult<Box<dyn Service>> {
        let sdk = factory.get_sdk(name)?;

        let service = match name {
            "authorization" => {
                let multi_sig_sdk = factory.get_sdk("multi_signature")?;
                Box::new(AuthorizationService::new(
                    sdk,
                    MultiSignatureService::new(multi_sig_sdk),
                )) as Box<dyn Service>
            }
            "asset" => Box::new(AssetService::new(sdk)) as Box<dyn Service>,
            "metadata" => Box::new(MetadataService::new(sdk)) as Box<dyn Service>,
            "multi_signature" => Box::new(MultiSignatureService::new(sdk)) as Box<dyn Service>,
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
            "authorization".to_owned(),
            "metadata".to_owned(),
            "multi_signature".to_owned(),
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

// Note: inject runnning_status
pub async fn run(listen_port: u16, seckey: String, sync: Sync) {
    let builder = MutaBuilder::new()
        .config_path(consts::CHAIN_CONFIG_PATH)
        .genesis_path(consts::CHAIN_GENESIS_PATH)
        .service_mapping(DefaultServiceMapping {});

    let muta = builder.build(listen_port).expect("build");
    muta.run(seckey, sync).await.expect("run");
}
