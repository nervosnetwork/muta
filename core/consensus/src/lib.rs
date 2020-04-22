use std::boxed::Box;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use derive_more::Display;
use futures::channel::mpsc::UnboundedSender;
use overlord::types::{Aggregates, Vote, HeightRange, ExecResult, TinyHex};
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
    Address as ProtoAddress, Block, BlockHeader, Bloom, Bytes, MerkleRoot, Metadata, Proof as ProtoProof, Receipt, SignedTransaction,
    TransactionRequest, Validator, Hash as ProtoHash, Pill
};
use protocol::{fixed_codec::FixedCodec, ProtocolResult};

struct Status {
    chain_id: ProtoHash,
    address: ProtoAddress,

    // update in every commit
    last_state_root: MerkleRoot,
    cycles_limit:    u64,
    tx_num_limit:    u64,
    max_tx_size:     u64,
    validators: Vec<Validator>,

    exemption_hash: HashSet<Bytes>,
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
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let (ordered_tx_hashes, propose_hashes) = self.mem_pool.package(ctx, self.status.cycles_limit, self.status.tx_num_limit).await?.clap();
        let order_root = Merkle::from_hashes(ordered_tx_hashes.clone()).get_root_hash();

        let mut block_states = block_states;
        block_states.sort_by(|a, b| a.height.partial_cmp(&b.height).unwrap());

        let state_root = if block_states.is_empty() {
            self.status.last_state_root.clone()
        } else {
            block_states.last().unwrap().state.state_root.clone()
        };

        let header = BlockHeader {
            chain_id: self.status.chain_id.clone(),
            pre_hash: ProtoHash::from_bytes(pre_hash)?,
            height,
            exec_height,
            timestamp,
            logs_bloom: block_states.iter().map(|stat| stat.state.log_bloom.clone()).collect(),
            order_root: order_root.unwrap_or_else(ProtoHash::from_empty),
            confirm_root: block_states.iter().map(|stat| stat.state.order_root.clone()).collect(),
            state_root,
            receipt_root: block_states.iter().map(|stat| stat.state.receipt_root.clone()).collect(),
            cycles_used: block_states.iter().map(|stat| stat.state.cycles_used).collect(),
            proposer: self.status.address.clone(),
            proof: into_proto_proof(pre_proof)?,
            validator_version: 0u64,
            validators: self.status.validators.clone(),
        };

        let block = Block {
            header,
            ordered_tx_hashes,
        };

        let pill = Pill {
            block,
            propose_hashes,
        };

        Ok(WrappedBlock(pill))
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
#[display(fmt = "{{ chain_id: {}, height: {}, exec_height: {}, order_tx_len: {}, propose_tx_len: {}, pre_hash: {}, timestamp: {}, state_root: {}, order_root: {}, confirm_root: {:?}, cycle_used: {:?}, proposer: {}, validator_version: {}, validators: {:?} }}",
"_0.block.header.chain_id.as_bytes().tiny_hex()", 
"_0.block.header.height",
"_0.block.header.exec_height",
"_0.block.ordered_tx_hashes.len()",
"_0.propose_hashes.len()",
"_0.block.header.pre_hash.as_bytes().tiny_hex()",
"_0.block.header.timestamp",
"_0.block.header.state_root.as_bytes().tiny_hex()",
"_0.block.header.order_root.as_bytes().tiny_hex()",
"_0.block.header.confirm_root.iter().map(|root| root.as_bytes().tiny_hex()).collect::<Vec<String>>()",
"_0.block.header.cycles_used",
"_0.block.header.proposer.as_bytes().tiny_hex()",
"_0.block.header.validator_version",
"_0.block.header.validators.iter().map(|v| format!(\"{{ address: {}, propose_w: {}, vote_w: {} }}\", v.address.as_bytes().tiny_hex(), v.propose_weight, v.vote_weight))",)]
struct WrappedBlock(Pill);

impl Blk for WrappedBlock {
    fn fixed_encode(&self) -> Result<Bytes, Box<dyn Error + Send>>{
        let encode = self.0.encode_fixed()?;
        Ok(encode)
    }

    fn fixed_decode(data: &Bytes) -> Result<Self, Box<dyn Error + Send>>{
        let pill = FixedCodec::decode_fixed(data.clone())?;
        Ok(WrappedBlock(pill))
    }

    fn get_block_hash(&self) -> Hash{
        // Todo: change return to Result<Hash, Err>
        ProtoHash::digest(self.fixed_encode().expect("fixed encode block failed")).as_bytes()
    }

    fn get_pre_hash(&self) -> Hash{
        self.0.block.header.pre_hash.as_bytes()
    }

    fn get_height(&self) -> Height{
        self.0.block.header.height
    }

    fn get_exec_height(&self) -> Height{
        self.0.block.header.exec_height
    }

    fn get_proof(&self) -> Proof{
        into_proof(self.0.block.header.proof.clone())
    }
}

#[derive(Clone, Debug, Default, Display)]
#[display(fmt = "{{ order_root: {}, state_root: {}, receipt_root: {}, cycle_used: {} }}",
"order_root.as_bytes().tiny_hex()",
"state_root.as_bytes().tiny_hex()",
"receipt_root.as_bytes().tiny_hex()",
cycles_used)]
struct WrappedExecResp {
    order_root: MerkleRoot,
    state_root: MerkleRoot,
    receipt_root: MerkleRoot,
    cycles_used: u64,
    log_bloom: Bloom,
}

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