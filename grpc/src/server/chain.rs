use std::marker::{Send, Sync};

use grpc::{RequestOptions, SingleResponse};

use crate::{
    grpc::GrpcChainService,
    proto::{blockchain::Block, chain::*},
    service::{ChainService, Context},
    ContextExchange, FutResponseExt,
};

pub struct ChainServer {
    server: ::grpc::Server,
}

impl_server!(ChainServer, GrpcChainImpl, CHAIN);

struct GrpcChainImpl<T> {
    core_srv: T,
}

impl<T: ChainService + Sync + Send + 'static> GrpcChainService for GrpcChainImpl<T> {
    fn add_block(&self, o: RequestOptions, block: Block) -> SingleResponse<AddBlockResp> {
        self.core_srv
            .add_block(Context::from_rpc_context(o), block)
            .into_single_resp()
    }

    fn get_block(&self, o: RequestOptions, block_height: BlockHeight) -> SingleResponse<BlockResp> {
        self.core_srv
            .get_block(Context::from_rpc_context(o), block_height)
            .into_single_resp()
    }

    fn get_receipt(
        &self,
        o: RequestOptions,
        tx_hash: TransactionHash,
    ) -> SingleResponse<ReceiptResp> {
        self.core_srv
            .get_receipt(Context::from_rpc_context(o), tx_hash)
            .into_single_resp()
    }

    fn get_transaction(
        &self,
        o: RequestOptions,
        hash: TransactionHash,
    ) -> SingleResponse<SignedTransactionResp> {
        self.core_srv
            .get_transaction(Context::from_rpc_context(o), hash)
            .into_single_resp()
    }

    fn call(&self, o: RequestOptions, data: Data) -> SingleResponse<CallResp> {
        self.core_srv
            .call(Context::from_rpc_context(o), data)
            .into_single_resp()
    }
}
