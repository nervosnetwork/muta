macro_rules! impl_client {
    ($client:ident, $host:ident, $port:ident) => {
        use mashup::*;

        mashup! {
            m["grpc_client"] = Grpc $client;
        }

        m!{
            use crate::service::error::ServiceError;

            impl $client {
                pub(crate) fn new() -> Result<Self, ServiceError> {
                    use std::sync::Arc;
                    use grpc::ClientStub;
                    use crate::{
                        grpc::"grpc_client",
                        common::{env, constant::{$host, $port}},
                        error::ServiceErrorExt,
                    };

                    let host = env::env_var($host)?;
                    let port = env::env_value::<u16>($port)?;
                    println!("{}", format!("connect to {}:{}", host, port));
                    let plain_client = grpc::Client::new_plain(&host, port, Default::default()).map_err(ServiceError::from_grpc_err)?;
                    let client = "grpc_client"::with_client(Arc::new(plain_client));

                    Ok(Self {
                        client,
                    })
                }
            }
        }
    };
}
