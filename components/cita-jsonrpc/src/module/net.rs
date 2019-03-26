use futures::future;
use jsonrpc_core::BoxFuture;
use jsonrpc_derive::rpc;
use jsonrpc_types::rpctypes::Quantity;

#[rpc]
pub trait Net {
    #[rpc(name = "peerCount")]
    fn peer_count(&self) -> BoxFuture<Quantity>;
}

pub struct NetRpcImpl;

impl Net for NetRpcImpl {
    fn peer_count(&self) -> BoxFuture<Quantity> {
        Box::new(future::ok(Quantity::new(123.into())))
    }
}
