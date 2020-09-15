use derive_more::{Display, From};
use protocol::traits::{SDKFactory, Service, ServiceMapping, ServiceSDK};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use asset::{AssetService, ASSET_SERVICE_NAME};
use authorization::{AuthorizationService, AUTHORIZATION_SERVICE_NAME};
use metadata::{MetadataService, METADATA_SERVICE_NAME};
use multi_signature::{MultiSignatureService, MULTI_SIG_SERVICE_NAME};
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
                .into());
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

pub fn main() {
    muta::run(
        DefaultServiceMapping,
        "muta-chain",
        "v0.2.0-rc.2.1",
        "Muta Dev <muta@nervos.org>",
        None,
    )
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
