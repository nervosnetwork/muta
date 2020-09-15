use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

use async_trait::async_trait;
use futures::lock::Mutex;
use overlord::types::{AggregatedSignature, Commit, Proof as OverlordProof};
use overlord::Consensus;

use common_crypto::BlsPrivateKey;
use protocol::fixed_codec::FixedCodec;
use protocol::traits::{
    CommonConsensusAdapter, ConsensusAdapter, Context, MessageTarget, MixedTxHashes, NodeInfo,
    TrustFeedback,
};
use protocol::types::{
    Address, Block, BlockHeader, Hash, Hex, MerkleRoot, Metadata, Pill, Proof, Receipt,
    SignedTransaction, Validator,
};
use protocol::{Bytes, ProtocolResult};

use crate::engine::ConsensusEngine;
use crate::fixed_types::FixedPill;
use crate::status::StatusAgent;
use crate::util::OverlordCrypto;
use crate::wal::{ConsensusWal, SignedTxsWAL};

use super::*;

static FULL_TXS_PATH: &str = "./free-space/engine/txs";
static FULL_CONSENSUS_PATH: &str = "./free-space/engine/consensus";

#[tokio::test]
async fn test_repetitive_commit() {
    let init_status = mock_current_status(1);
    let engine = init_engine(init_status.clone());

    let block = mock_block_from_status(&init_status);

    let res = engine
        .commit(Context::new(), 11, mock_commit(block.clone()))
        .await;
    assert!(res.is_ok());

    let status = engine.get_current_status();

    let res = engine
        .commit(Context::new(), 11, mock_commit(block.clone()))
        .await;
    assert!(res.is_err());

    assert_eq!(status, engine.get_current_status());
}

fn mock_commit(block: Block) -> Commit<FixedPill> {
    let pill = Pill {
        block:          block.clone(),
        propose_hashes: vec![],
    };
    Commit {
        height:  11,
        content: FixedPill { inner: pill },
        proof:   OverlordProof {
            height:     11,
            round:      0,
            block_hash: Hash::digest(block.header.encode_fixed().unwrap()).as_bytes(),
            signature:  AggregatedSignature {
                signature:      get_random_bytes(32),
                address_bitmap: get_random_bytes(10),
            },
        },
    }
}

fn init_engine(init_status: CurrentConsensusStatus) -> ConsensusEngine<MockConsensusAdapter> {
    ConsensusEngine::new(
        StatusAgent::new(init_status),
        mock_node_info(),
        Arc::new(SignedTxsWAL::new(FULL_TXS_PATH)),
        Arc::new(MockConsensusAdapter {}),
        Arc::new(init_crypto()),
        Arc::new(Mutex::new(())),
        Arc::new(ConsensusWal::new(FULL_CONSENSUS_PATH)),
    )
}

fn init_crypto() -> OverlordCrypto {
    let mut priv_key = Vec::new();
    priv_key.extend_from_slice(&[0u8; 16]);
    let mut tmp =
        hex::decode("45c56be699dca666191ad3446897e0f480da234da896270202514a0e1a587c3f").unwrap();
    priv_key.append(&mut tmp);

    OverlordCrypto::new(
        BlsPrivateKey::try_from(priv_key.as_ref()).unwrap(),
        HashMap::new(),
        std::str::from_utf8(hex::decode("").unwrap().as_ref())
            .unwrap()
            .into(),
    )
}

fn mock_node_info() -> NodeInfo {
    NodeInfo {
        self_pub_key: mock_pub_key().decode(),
        chain_id:     mock_hash(),
        self_address: mock_address(),
    }
}

fn mock_metadata() -> Metadata {
    Metadata {
        chain_id:           mock_hash(),
        bech32_address_hrp: "muta".to_owned(),
        common_ref:         Hex::from_string("0x703873635a6b51513451".to_string()).unwrap(),
        timeout_gap:        20,
        cycles_limit:       600000,
        cycles_price:       1,
        interval:           3000,
        verifier_list:      vec![],
        propose_ratio:      3,
        prevote_ratio:      3,
        precommit_ratio:    3,
        brake_ratio:        3,
        tx_num_limit:       3,
        max_tx_size:        3000,
    }
}

pub struct MockConsensusAdapter;

#[async_trait]
impl CommonConsensusAdapter for MockConsensusAdapter {
    async fn save_block(&self, _ctx: Context, _block: Block) -> ProtocolResult<()> {
        Ok(())
    }

    async fn save_proof(&self, _ctx: Context, _proof: Proof) -> ProtocolResult<()> {
        Ok(())
    }

    async fn save_signed_txs(
        &self,
        _ctx: Context,
        _block_height: u64,
        _signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    async fn save_receipts(
        &self,
        _ctx: Context,
        _height: u64,
        _receipts: Vec<Receipt>,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    async fn flush_mempool(
        &self,
        _ctx: Context,
        _ordered_tx_hashes: &[Hash],
    ) -> ProtocolResult<()> {
        Ok(())
    }

    async fn get_block_by_height(&self, _ctx: Context, _height: u64) -> ProtocolResult<Block> {
        unimplemented!()
    }

    async fn get_block_header_by_height(
        &self,
        _ctx: Context,
        _height: u64,
    ) -> ProtocolResult<BlockHeader> {
        unimplemented!()
    }

    async fn get_current_height(&self, _ctx: Context) -> ProtocolResult<u64> {
        Ok(10)
    }

    async fn get_txs_from_storage(
        &self,
        _ctx: Context,
        _tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        unimplemented!()
    }

    async fn verify_block_header(&self, _ctx: Context, _block: &Block) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn verify_proof(
        &self,
        _ctx: Context,
        _block_header: &BlockHeader,
        _proof: &Proof,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    async fn broadcast_height(&self, _ctx: Context, _height: u64) -> ProtocolResult<()> {
        Ok(())
    }

    fn get_metadata(
        &self,
        _context: Context,
        _state_root: MerkleRoot,
        _height: u64,
        _timestamp: u64,
        _proposer: Address,
    ) -> ProtocolResult<Metadata> {
        Ok(mock_metadata())
    }

    fn report_bad(&self, _ctx: Context, _feedback: TrustFeedback) {}

    fn set_args(
        &self,
        _context: Context,
        _timeout_gap: u64,
        _cycles_limit: u64,
        _max_tx_size: u64,
    ) {
    }

    fn tag_consensus(&self, _ctx: Context, _peer_ids: Vec<Bytes>) -> ProtocolResult<()> {
        Ok(())
    }

    fn verify_proof_signature(
        &self,
        _ctx: Context,
        _block_height: u64,
        _vote_hash: Bytes,
        _aggregated_signature_bytes: Bytes,
        _vote_pubkeys: Vec<Hex>,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    fn verify_proof_weight(
        &self,
        _ctx: Context,
        _block_height: u64,
        _weight_map: HashMap<Bytes, u32>,
        _signed_voters: Vec<Bytes>,
    ) -> ProtocolResult<()> {
        Ok(())
    }
}

#[async_trait]
impl ConsensusAdapter for MockConsensusAdapter {
    async fn get_txs_from_mempool(
        &self,
        _ctx: Context,
        _height: u64,
        _cycles_limit: u64,
        _tx_num_limit: u64,
    ) -> ProtocolResult<MixedTxHashes> {
        unimplemented!()
    }

    async fn sync_txs(&self, _ctx: Context, _txs: Vec<Hash>) -> ProtocolResult<()> {
        Ok(())
    }

    async fn get_full_txs(
        &self,
        _ctx: Context,
        _txs: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        Ok(vec![])
    }

    async fn transmit(
        &self,
        _ctx: Context,
        _msg: Vec<u8>,
        _end: &str,
        _target: MessageTarget,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    async fn execute(
        &self,
        _ctx: Context,
        _chain_id: Hash,
        _order_root: MerkleRoot,
        _height: u64,
        _cycles_price: u64,
        _proposer: Address,
        _block_hash: Hash,
        _signed_txs: Vec<SignedTransaction>,
        _cycles_limit: u64,
        _timestamp: u64,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    async fn get_last_validators(
        &self,
        _ctx: Context,
        _height: u64,
    ) -> ProtocolResult<Vec<Validator>> {
        unimplemented!()
    }

    async fn pull_block(&self, _ctx: Context, _height: u64, _end: &str) -> ProtocolResult<Block> {
        unimplemented!()
    }

    async fn get_current_height(&self, _ctx: Context) -> ProtocolResult<u64> {
        Ok(10)
    }

    async fn verify_txs(&self, _ctx: Context, _height: u64, _txs: &[Hash]) -> ProtocolResult<()> {
        Ok(())
    }
}
