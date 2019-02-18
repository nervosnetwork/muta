macro_rules! impl_server {
    ($server:ident, $impl:ident, $env_prefix:ident) => {
        use mashup::*;
        use crate::common::env;

        mashup! {
            m["service"] = $server Service;
            m["grpc-server"] = Grpc $server;
            m["env_host"] = $env_prefix _SERVER_HOST;
            m["env_port"] = $env_prefix _SERVER_PORT;
            m["env_threads"] = $env_prefix _SERVER_THREADS;
        }

        m! {
            use std::net::SocketAddr;
            use crate::{
                service::error::ServiceError,
                grpc::{"grpc-server", "service"}
            };

            impl $server {
                pub fn new<T: "service" + Sync + Send + 'static>(serv: T) -> Result<Self, ServiceError> {
                    use crate::{
                        common::constant::{"env_host", "env_port", "env_threads"},
                        error::ServiceErrorExt,
                    };

                    let grpc_impl =  $impl {
                        core_srv: serv
                    };

                    let host = env::env_var("env_host")?;
                    let port = env::env_value::<u16>("env_port")?;
                    let threads = env::env_value::<usize>("env_threads").unwrap_or(num_cpus::get());
                    let addr =  format!("{}:{}", host, port);
                    let addr: SocketAddr = addr.parse().
                        map_err(|e: std::net::AddrParseError| ServiceError::Panic(e.to_string()))?;
                    let mut builder = grpc::ServerBuilder::new_plain();
                    builder.http.set_addr(addr).map_err(|e| ServiceError::Panic(format!("{}", e)))?;
                    builder.add_service("grpc-server"::new_service_def(grpc_impl));
                    builder.http.set_cpu_pool_threads(threads);

                    let server = builder.build().map_err(ServiceError::from_grpc_err)?;
                    Ok(Self {
                        server,
                    })
                }

                pub fn local_addr(&self) -> Option<&SocketAddr> {
                    use httpbis::AnySocketAddr;

                    match self.server.local_addr() {
                        AnySocketAddr::Inet(socket_addr) => Some(socket_addr),
                        AnySocketAddr::Unix(_) => None,
                    }
                }

                pub fn is_alive(&self) -> bool {
                    self.server.is_alive()
                }
            }
        }
    };
}
