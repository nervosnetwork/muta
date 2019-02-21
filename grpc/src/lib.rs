#![recursion_limit = "512"]

macro_rules! rename_import {
    ($module:ident, $name:ident) => (
        pub mod $module {
        use mashup::*;

            mashup! {
                import["grpc_module"] = $module _grpc;
                import["service"] = $name Service;
                import["renamed_service"] = Grpc $name Service;
                import["server"] = $name ServiceServer;
                import["renamed_server"] = Grpc $name Server;
                import["client"] = $name ServiceClient;
                import["renamed_client"] = Grpc $name Client;
                import["renamed_server_service"] = $name ServerService;
            }

            import! {
                pub use muta_proto::"grpc_module"::{"service" as "renamed_service", "server" as "renamed_server", "client" as "renamed_client"};
                pub use crate::service::"service" as "renamed_server_service";
            }
        }
        pub use self::$module::*;
    )
}

pub(crate) mod proto {
    pub use muta_proto::*;
}

pub(crate) mod service {
    pub use muta_service::*;
}

// for example: PoolService as GrpcPoolService, PoolServiceServer as GrpcPoolServer
pub(crate) mod grpc {
    rename_import!(pool, Pool);
    rename_import!(chain, Chain);
    rename_import!(consensus, Consensus);
    rename_import!(executor, Executor);
    rename_import!(network, Network);
    rename_import!(sync, Sync);
}

pub mod server {
    #[macro_use]
    pub(crate) mod macros;
    pub mod chain;
    pub mod consensus;
    pub mod executor;
    pub mod network;
    pub mod pool;
    pub mod sync;
}

pub mod client {
    #[macro_use]
    pub(crate) mod macros;
    pub mod chain;
    pub mod consensus;
    pub mod executor;
    pub mod network;
    pub mod pool;
    pub mod sync;
}

pub mod client_container;

pub(crate) mod common {
    pub mod constant;
    pub mod env;
}

pub(crate) mod context;
pub(crate) mod error;
pub(crate) mod response;

pub(crate) use context::ContextExchange;
pub(crate) use response::FutResponseExt;
pub(crate) use response::SingleResponseExt;
