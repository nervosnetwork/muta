pub mod proto {
    pub use umaru_proto::blockchain;
    pub use umaru_proto::common;

    pub use umaru_proto::chain;
    pub use umaru_proto::consensus;
    pub use umaru_proto::executor;
    pub use umaru_proto::pool;
    pub use umaru_proto::sync;
}

pub mod service {
    pub use umaru_service::error;
    pub use umaru_service::Context;
    pub use umaru_service::FutResponse;

    pub use umaru_service::ChainService;
    pub use umaru_service::ConsensusService;
    pub use umaru_service::ExecutorService;
    pub use umaru_service::NetworkService;
    pub use umaru_service::PoolService;
    pub use umaru_service::SyncService;
}

#[cfg(feature = "with-grpc")]
pub mod server {
    pub use umaru_grpc::server::*;

    pub use chain::ChainServer;
    pub use consensus::ConsensusServer;
    pub use executor::ExecutorServer;
    pub use network::NetworkServer;
    pub use pool::PoolServer;
    pub use sync::SyncServer;
}

#[cfg(feature = "with-grpc")]
pub mod client {
    pub use umaru_grpc::client_container::*;
}

pub mod prelude {
    pub use crate::service::Context;
    pub use crate::service::FutResponse;
}
