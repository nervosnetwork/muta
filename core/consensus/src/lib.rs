use std::boxed::Box;
use std::collections::HashMap;
use std::error::Error;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use derive_more::Display;
use futures::channel::mpsc::UnboundedSender;
use overlord::types::{Aggregates, Vote, HeightRange, ExecResult};
use overlord::{Address, Adapter, Blk, DefaultCrypto, Height, Proof, BlockState, Hash, St, OverlordMsg, OverlordError};
use parking_lot::RwLock;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use common_merkle::Merkle;

use protocol::traits::{
    CommonConsensusAdapter, ConsensusAdapter, Context, ExecutorFactory, ExecutorParams,
    ExecutorResp, Gossip, MemPool, MessageTarget, MixedTxHashes, Priority, Rpc, ServiceMapping,
    Storage, SynchronizationAdapter,
};
use protocol::types::{
    Block, Bytes, MerkleRoot, Metadata, Proof as ProtoProof, Receipt, SignedTransaction,
    TransactionRequest, Validator, Hash as ProtoHash
};
use protocol::{fixed_codec::FixedCodec, ProtocolResult};

struct Status {
    last_state_root: MerkleRoot,
}

pub struct OverlordAdapter<
    EF: ExecutorFactory<DB, S, Mapping>,
    G: Gossip,
    M: MemPool,
    R: Rpc,
    S: Storage,
    DB: cita_trie::DB,
    Mapping: ServiceMapping,
> {
    status:          Status,
    rpc:             Arc<R>,
    network:         Arc<G>,
    mem_pool:        Arc<M>,
    storage:         Arc<S>,
    trie_db:         Arc<DB>,
    service_mapping: Arc<Mapping>,

    phantom:        PhantomData<EF>,
}

#[async_trait]
impl<EF, G, M, R, S, DB, Mapping> Adapter<WrappedBlock, WrappedExecResp> for OverlordAdapter<EF, G, M, R, S, DB, Mapping>
where
    EF: ExecutorFactory<DB, S, Mapping> + 'static,
    G: Gossip + Sync + Send + 'static,
    R: Rpc + Sync + Send + 'static,
    M: MemPool + 'static,
    S: Storage + 'static,
    DB: cita_trie::DB + 'static,
    Mapping: ServiceMapping + 'static,
{
    type CryptoImpl = DefaultCrypto;

    async fn create_block(
        &self,
        ctx: Context,
        height: Height,
        exec_height: Height,
        pre_hash: Hash,
        pre_proof: Proof,
        block_states: Vec<BlockState<WrappedExecResp>>,
    ) -> Result<WrappedBlock, Box<dyn Error + Send>> {
        Ok(WrappedBlock::default())
    }

    async fn check_block(
        &self,
        ctx: Context,
        block: &WrappedBlock,
        block_states: &[BlockState<WrappedExecResp>],
    ) -> Result<(), Box<dyn Error + Send>> {
        Ok(())
    }

    async fn fetch_full_block(
        &self,
        ctx: Context,
        block: WrappedBlock,
    ) -> Result<Bytes, Box<dyn Error + Send>> {
        Ok(Bytes::default())
    }

    async fn save_and_exec_block_with_proof(
        &self,
        ctx: Context,
        height: Height,
        full_block: Bytes,
        proof: Proof,
    ) -> Result<ExecResult<WrappedExecResp>, Box<dyn Error + Send>> {
        Ok(ExecResult::default())
    }

    async fn register_network(
        &self,
        _ctx: Context,
        sender: UnboundedSender<(Context, OverlordMsg<WrappedBlock>)>,
    ) {
    }

    async fn broadcast(
        &self,
        ctx: Context,
        msg: OverlordMsg<WrappedBlock>,
    ) -> Result<(), Box<dyn Error + Send>> {
        Ok(())
    }

    async fn transmit(
        &self,
        ctx: Context,
        to: Address,
        msg: OverlordMsg<WrappedBlock>,
    ) -> Result<(), Box<dyn Error + Send>> {
        Ok(())
    }

    /// should return empty vec if the required blocks are not exist
    async fn get_block_with_proofs(
        &self,
        ctx: Context,
        height_range: HeightRange,
    ) -> Result<Vec<(WrappedBlock, Proof)>, Box<dyn Error + Send>> {
        Ok(vec![])
    }

    async fn get_latest_height(&self, ctx: Context) -> Result<Height, Box<dyn Error + Send>> {
        Ok(0)
    }

    async fn handle_error(&self, ctx: Context, err: OverlordError) {}
}

#[derive(Clone, Debug, Default, Display, PartialEq, Eq)]
struct WrappedBlock(Block);

impl Blk for WrappedBlock {
    fn fixed_encode(&self) -> Result<Bytes, Box<dyn Error + Send>>{
        let encode = self.0.encode_fixed()?;
        Ok(encode)
    }

    fn fixed_decode(data: &Bytes) -> Result<Self, Box<dyn Error + Send>>{
        let block = FixedCodec::decode_fixed(data.clone())?;
        Ok(WrappedBlock(block))
    }

    fn get_block_hash(&self) -> Hash{
        // Todo: change return to Result<Hash, Err>
        ProtoHash::digest(self.fixed_encode().expect("fixed encode block failed")).as_bytes()
    }

    fn get_pre_hash(&self) -> Hash{
        self.0.header.pre_hash.as_bytes()
    }

    fn get_height(&self) -> Height{
        self.0.header.height
    }

    fn get_exec_height(&self) -> Height{
        self.0.header.exec_height
    }

    fn get_proof(&self) -> Proof{
        // Todo: change return to Result<Hash, Err>
        into_proof(self.0.header.proof.clone())
    }
}

#[derive(Clone, Debug, Default, Display)]
struct WrappedExecResp(ExecutorResp);

impl St for WrappedExecResp {}

fn into_proof(proof: ProtoProof) -> Proof {
    let vote = Vote::new(proof.height, proof.round, proof.block_hash.as_bytes());
    let aggregates = Aggregates::new(proof.bitmap, proof.signature);
    Proof::new(vote, aggregates)
}

fn into_proto_proof(proof: Proof) -> ProtocolResult<ProtoProof> {
    let proof = ProtoProof {
        height: proof.vote.height,
        round: proof.vote.round,
        block_hash: ProtoHash::from_bytes(proof.vote.block_hash)?,
        signature: proof.aggregates.signature,
        bitmap: proof.aggregates.address_bitmap,
    };
    Ok(proof)
}