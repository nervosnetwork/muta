pub mod proto {
    pub use muta_proto::blockchain;
    pub use muta_proto::common;

    pub use muta_proto::chain;
    pub use muta_proto::consensus;
    pub use muta_proto::executor;
    pub use muta_proto::pool;
    pub use muta_proto::sync;
}

pub mod service {
    pub use muta_service::error;
    pub use muta_service::Context;
    pub use muta_service::FutResponse;

    pub use muta_service::ChainService;
    pub use muta_service::ConsensusService;
    pub use muta_service::ExecutorService;
    pub use muta_service::NetworkService;
    pub use muta_service::PoolService;
    pub use muta_service::SyncService;
}

#[cfg(feature = "with-grpc")]
pub mod server {
    pub use muta_grpc::server;

    pub use server::chain::ChainServer;
    pub use server::consensus::ConsensusServer;
    pub use server::executor::ExecutorServer;
    pub use server::network::NetworkServer;
    pub use server::pool::PoolServer;
    pub use server::sync::SyncServer;
}

#[cfg(feature = "with-grpc")]
pub use muta_grpc::client_container as client;
