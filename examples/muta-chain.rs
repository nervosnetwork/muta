use derive_more::{Display, From};
use protocol::traits::{SDKFactory, Service, ServiceMapping, ServiceSDK};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use asset::{AssetService, ASSET_SERVICE_NAME};
use authorization::{AuthorizationService, AUTHORIZATION_SERVICE_NAME};
use metadata::{MetadataService, METADATA_SERVICE_NAME};
use multi_signature::{MultiSignatureService, MULTI_SIG_SERVICE_NAME};
use muta::MutaBuilder;
use util::{UtilService, UTIL_SERVICE_NAME};

struct DefaultServiceMapping;

impl ServiceMapping for DefaultServiceMapping {
    fn get_service<SDK: 'static + ServiceSDK, Factory: SDKFactory<SDK>>(
        &self,
        name: &str,
        factory: &Factory,
    ) -> ProtocolResult<Box<dyn Service>> {
        let sdk = factory.get_sdk(name)?;
        let service = match name {
            AUTHORIZATION_SERVICE_NAME => {
                let multi_sig_sdk = factory.get_sdk("multi_signature")?;
                Box::new(AuthorizationService::new(
                    sdk,
                    MultiSignatureService::new(multi_sig_sdk),
                )) as Box<dyn Service>
            }
            ASSET_SERVICE_NAME => Box::new(AssetService::new(sdk)) as Box<dyn Service>,
            METADATA_SERVICE_NAME => Box::new(MetadataService::new(sdk)) as Box<dyn Service>,
            MULTI_SIG_SERVICE_NAME => Box::new(MultiSignatureService::new(sdk)) as Box<dyn Service>,
            UTIL_SERVICE_NAME => Box::new(UtilService::new(sdk)) as Box<dyn Service>,
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
            ASSET_SERVICE_NAME.to_owned(),
            AUTHORIZATION_SERVICE_NAME.to_owned(),
            METADATA_SERVICE_NAME.to_owned(),
            MULTI_SIG_SERVICE_NAME.to_owned(),
            UTIL_SERVICE_NAME.to_owned(),
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
