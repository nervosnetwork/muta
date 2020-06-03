use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::sync::Arc;

use async_trait::async_trait;
use futures::executor::block_on;
use futures::lock::Mutex;
use parking_lot::RwLock;
use rlp::encode;

use common_crypto::{
    BlsCommonReference, BlsPrivateKey, BlsPublicKey, HashValue, PrivateKey, PublicKey,
    Secp256k1PrivateKey, Secp256k1PublicKey, Signature, ToPublicKey,
};
use common_merkle::Merkle;

use protocol::fixed_codec::FixedCodec;
use protocol::traits::{CommonConsensusAdapter, Synchronization, SynchronizationAdapter};
use protocol::traits::{Context, ExecutorParams, ExecutorResp, ServiceResponse, TrustFeedback};
use protocol::types::{
    Address, Block, BlockHeader, Bytes, Hash, Hex, MerkleRoot, Metadata, Proof, RawTransaction,
    Receipt, ReceiptResponse, SignedTransaction, TransactionRequest, Validator, ValidatorExtend,
};
use protocol::ProtocolResult;

use crate::status::{CurrentConsensusStatus, StatusAgent};
use crate::synchronization::{OverlordSynchronization, RichBlock};
use crate::util::{convert_hex_to_bls_pubkeys, digest_signed_transactions, OverlordCrypto};
use crate::BlockHeaderField::{PreviousBlockHash, ProofHash, Proposer};
use crate::BlockProofField::{BitMap, HashMismatch, HeightMismatch, WeightNotFound};
use crate::{BlockHeaderField, BlockProofField, ConsensusError};
use bit_vec::BitVec;
use overlord::types::{AggregatedSignature, AggregatedVote, Node, SignedVote, Vote, VoteType};
use overlord::{extract_voters, Crypto};

// Test the blocks gap from 1 to 4.
#[test]
fn sync_gap_test() {
    for gap in [1, 2, 3, 4].iter() {
        let key_tool = get_mock_key_tool();

        let max_height = 10 * *gap;

        let list_rich_block = mock_chained_rich_block(max_height, *gap, &key_tool);

        let remote_blocks = gen_remote_block_hashmap(list_rich_block.0.clone());
        let remote_proofs = gen_remote_proof_hashmap(list_rich_block.1.clone());
        let genesis_block = remote_blocks.read().get(&0).unwrap().clone();

        let local_blocks = Arc::new(RwLock::new(HashMap::new()));
        local_blocks
            .write()
            .insert(genesis_block.header.height, genesis_block.clone());

        let local_transactions = Arc::new(RwLock::new(HashMap::new()));
        let remote_transactions = gen_remote_tx_hashmap(list_rich_block.0.clone());

        let adapter = Arc::new(MockCommonConsensusAdapter::new(
            0,
            local_blocks,
            remote_blocks,
            remote_proofs,
            local_transactions,
            remote_transactions,
            Arc::clone(&key_tool.overlord_crypto),
        ));
        let block_hash = Hash::digest(genesis_block.header.encode_fixed().unwrap());
        let status = CurrentConsensusStatus {
            cycles_price:                1,
            cycles_limit:                300_000_000,
            latest_committed_height:     genesis_block.header.height,
            exec_height:                 genesis_block.header.exec_height,
            current_hash:                block_hash,
            list_logs_bloom:             vec![],
            list_confirm_root:           vec![],
            latest_committed_state_root: genesis_block.header.state_root.clone(),
            list_state_root:             vec![],
            list_receipt_root:           vec![],
            list_cycles_used:            vec![],
            current_proof:               genesis_block.header.proof,
            validators:                  genesis_block.header.validators,
            consensus_interval:          3000,
            propose_ratio:               15,
            prevote_ratio:               10,
            precommit_ratio:             10,
            brake_ratio:                 3,
            tx_num_limit:                20000,
            max_tx_size:                 1_073_741_824,
        };
        let status_agent = StatusAgent::new(status);
        let lock = Arc::new(Mutex::new(()));
        let sync = OverlordSynchronization::<_>::new(
            5000,
            Arc::clone(&adapter),
            status_agent.clone(),
            Arc::new(mock_crypto()),
            lock,
        );

        // simulate to get a block
        block_on(sync.receive_remote_block(Context::new(), max_height / 2)).unwrap();

        // get the current consensus status to check if the test works fine
        let status = status_agent.to_inner();
        let block =
            block_on(adapter.get_block_by_height(Context::new(), status.latest_committed_height))
                .unwrap();
        assert_sync(status, block);

        block_on(sync.receive_remote_block(Context::new(), max_height)).unwrap();
        let status = status_agent.to_inner();
        let block =
            block_on(adapter.get_block_by_height(Context::new(), status.latest_committed_height))
                .unwrap();
        assert_sync(status, block);

        let status = status_agent.to_inner();
        // status.current_height is consensus-ed height
        assert_eq!(status.latest_committed_height, max_height);
    }
}

pub type SafeHashMap<K, V> = Arc<RwLock<HashMap<K, V>>>;

pub struct MockCommonConsensusAdapter {
    latest_height:       RwLock<u64>,
    local_blocks:        SafeHashMap<u64, Block>,
    remote_blocks:       SafeHashMap<u64, Block>,
    remote_proofs:       SafeHashMap<u64, Proof>,
    local_transactions:  SafeHashMap<Hash, SignedTransaction>,
    remote_transactions: SafeHashMap<Hash, SignedTransaction>,
    crypto:              Arc<OverlordCrypto>,
}

impl MockCommonConsensusAdapter {
    pub fn new(
        latest_height: u64,
        local_blocks: SafeHashMap<u64, Block>,
        remote_blocks: SafeHashMap<u64, Block>,
        remote_proofs: SafeHashMap<u64, Proof>,
        local_transactions: SafeHashMap<Hash, SignedTransaction>,
        remote_transactions: SafeHashMap<Hash, SignedTransaction>,
        crypto: Arc<OverlordCrypto>,
    ) -> Self {
        Self {
            latest_height: RwLock::new(latest_height),
            local_blocks,
            remote_blocks,
            remote_proofs,
            local_transactions,
            remote_transactions,
            crypto,
        }
    }
}

#[async_trait]
impl SynchronizationAdapter for MockCommonConsensusAdapter {
    fn update_status(
        &self,
        _: Context,
        _: u64,
        _: u64,
        _: u64,
        _: u64,
        _: u64,
        _: u64,
        _: Vec<Validator>,
    ) -> ProtocolResult<()> {
        Ok(())
    }

    fn sync_exec(
        &self,
        _: Context,
        params: &ExecutorParams,
        txs: &[SignedTransaction],
    ) -> ProtocolResult<ExecutorResp> {
        Ok(exec_txs(params.height, txs).0)
    }

    /// Pull some blocks from other nodes from `begin` to `end`.
    async fn get_block_from_remote(&self, _: Context, height: u64) -> ProtocolResult<Block> {
        Ok(self.remote_blocks.read().get(&height).unwrap().clone())
    }

    /// Pull signed transactions corresponding to the given hashes from other
    /// nodes.
    async fn get_txs_from_remote(
        &self,
        _: Context,
        _: u64,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let map = self.remote_transactions.read();
        let mut txs = vec![];

        for hash in tx_hashes.iter() {
            let tx = map.get(hash).unwrap();
            txs.push(tx.clone())
        }

        Ok(txs)
    }

    async fn get_proof_from_remote(&self, _: Context, height: u64) -> ProtocolResult<Proof> {
        Ok(self.remote_proofs.read().get(&height).unwrap().clone())
    }
}

#[async_trait]
impl CommonConsensusAdapter for MockCommonConsensusAdapter {
    /// Save a block to the database.
    async fn save_block(&self, _: Context, block: Block) -> ProtocolResult<()> {
        self.local_blocks.write().insert(block.header.height, block);
        let mut height = self.latest_height.write();
        *height += 1;
        Ok(())
    }

    async fn save_proof(&self, _: Context, _: Proof) -> ProtocolResult<()> {
        Ok(())
    }

    /// Save some signed transactions to the database.
    async fn save_signed_txs(
        &self,
        _: Context,
        _block_height: u64,
        signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let mut map = self.local_transactions.write();
        for tx in signed_txs.into_iter() {
            map.insert(tx.tx_hash.clone(), tx);
        }
        Ok(())
    }

    async fn save_receipts(&self, _: Context, _: u64, _: Vec<Receipt>) -> ProtocolResult<()> {
        Ok(())
    }

    /// Flush the given transactions in the mempool.
    async fn flush_mempool(&self, _: Context, _: &[Hash]) -> ProtocolResult<()> {
        Ok(())
    }

    /// Get a block corresponding to the given height.
    async fn get_block_by_height(&self, _: Context, height: u64) -> ProtocolResult<Block> {
        Ok(self.local_blocks.read().get(&height).unwrap().clone())
    }

    /// Get the current height from storage.
    async fn get_current_height(&self, _: Context) -> ProtocolResult<u64> {
        Ok(*self.latest_height.read())
    }

    async fn get_txs_from_storage(
        &self,
        _: Context,
        tx_hashes: &[Hash],
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let map = self.local_transactions.read();
        let mut txs = vec![];

        for hash in tx_hashes.iter() {
            let tx = map.get(hash).unwrap();
            txs.push(tx.clone())
        }

        Ok(txs)
    }

    async fn broadcast_height(&self, _: Context, _: u64) -> ProtocolResult<()> {
        Ok(())
    }

    fn get_metadata(
        &self,
        _context: Context,
        _state_root: MerkleRoot,
        _height: u64,
        _timestamp: u64,
    ) -> ProtocolResult<Metadata> {
        Ok(Metadata {
            chain_id:        Hash::from_empty(),
            common_ref:      Hex::from_string("0x3453376d613471795964".to_string()).unwrap(),
            timeout_gap:     20,
            cycles_limit:    9999,
            cycles_price:    1,
            interval:        3000,
            verifier_list:   mock_verifier_list(),
            propose_ratio:   10,
            prevote_ratio:   10,
            precommit_ratio: 10,
            brake_ratio:     10,
            tx_num_limit:    20000,
            max_tx_size:     1_073_741_824,
        })
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

    /// this function verify all info in header except proof and roots
    async fn verify_block_header(&self, ctx: Context, block: Block) -> ProtocolResult<()> {
        let previous_block = self
            .get_block_by_height(ctx.clone(), block.header.height - 1)
            .await?;

        let previous_block_hash = Hash::digest(previous_block.header.encode_fixed()?);

        if previous_block_hash != block.header.prev_hash {
            log::error!(
                "[consensus] verify_block_header, previous_block_hash: {:?}, block.header.prev_hash: {:?}",
                previous_block_hash,
                block.header.prev_hash
            );
            return Err(
                ConsensusError::VerifyBlockHeader(block.header.height, PreviousBlockHash).into(),
            );
        }

        // the block 0 and 1 's proof is consensus-ed by community
        if block.header.height > 1u64 && block.header.prev_hash != block.header.proof.block_hash {
            log::error!(
                "[consensus] verify_block_header, verifying_block : {:?}",
                block
            );
            return Err(ConsensusError::VerifyBlockHeader(block.header.height, ProofHash).into());
        }

        // verify proposer and validators
        let previous_metadata = self.get_metadata(
            ctx,
            previous_block.header.state_root.clone(),
            previous_block.header.height,
            previous_block.header.timestamp,
        )?;

        let authority_map = previous_metadata
            .verifier_list
            .into_iter()
            .map(|v| {
                let address = v.address.as_bytes();
                let node = Node {
                    address:        v.address.as_bytes(),
                    propose_weight: v.propose_weight,
                    vote_weight:    v.vote_weight,
                };
                (address, node)
            })
            .collect::<HashMap<_, _>>();

        // check proposer
        if block.header.height != 0
            && !authority_map.contains_key(&block.header.proposer.as_bytes())
        {
            log::error!(
                "[consensus] verify_block_header, block.header.proposer: {:?}, authority_map: {:?}",
                block.header.proposer,
                authority_map
            );
            return Err(ConsensusError::VerifyBlockHeader(block.header.height, Proposer).into());
        }

        // check validators
        for validator in block.header.validators.iter() {
            if !authority_map.contains_key(&validator.address.as_bytes()) {
                log::error!(
                    "[consensus] verify_block_header, validator.address: {:?}, authority_map: {:?}",
                    validator.address,
                    authority_map
                );
                return Err(ConsensusError::VerifyBlockHeader(
                    block.header.height,
                    BlockHeaderField::Validator,
                )
                .into());
            } else {
                let node = authority_map.get(&validator.address.as_bytes()).unwrap();

                if node.vote_weight != validator.vote_weight
                    || node.propose_weight != validator.vote_weight
                {
                    log::error!(
                        "[consensus] verify_block_header, validator.address: {:?}, authority_map: {:?}",
                        validator.address,
                        authority_map
                    );
                    return Err(ConsensusError::VerifyBlockHeader(
                        block.header.height,
                        BlockHeaderField::Weight,
                    )
                    .into());
                }
            }
        }

        Ok(())
    }

    async fn verify_proof(&self, ctx: Context, block: Block, proof: Proof) -> ProtocolResult<()> {
        // the block 0 has no proof, which is consensus-ed by community, not by chain

        if block.header.height == 0 {
            return Ok(());
        };

        if block.header.height != proof.height {
            log::error!(
                "[consensus] verify_proof, block.header.height: {}, proof.height: {}",
                block.header.height,
                proof.height
            );
            return Err(ConsensusError::VerifyProof(
                block.header.height,
                HeightMismatch(block.header.height, proof.height),
            )
            .into());
        }

        let blockhash = Hash::digest(block.header.clone().encode_fixed()?);

        if blockhash != proof.block_hash {
            log::error!(
                "[consensus] verify_proof, blockhash: {:?}, proof.block_hash: {:?}",
                blockhash,
                proof.block_hash
            );
            return Err(ConsensusError::VerifyProof(block.header.height, HashMismatch).into());
        }

        let previous_block = self
            .get_block_by_height(ctx.clone(), block.header.height - 1)
            .await?;
        // the auth_list for the target should comes from previous height
        let metadata = self.get_metadata(
            ctx.clone(),
            previous_block.header.state_root.clone(),
            previous_block.header.height,
            previous_block.header.timestamp,
        )?;

        let mut authority_list = metadata
            .verifier_list
            .iter()
            .map(|v| Node {
                address:        v.address.as_bytes(),
                propose_weight: v.propose_weight,
                vote_weight:    v.vote_weight,
            })
            .collect::<Vec<Node>>();

        let signed_voters = extract_voters(&mut authority_list, &proof.bitmap).map_err(|_| {
            log::error!("[consensus] extract_voters fails, bitmap error");
            ConsensusError::VerifyProof(block.header.height, BitMap)
        })?;

        let vote = Vote {
            height:     proof.height,
            round:      proof.round,
            vote_type:  VoteType::Precommit,
            block_hash: proof.block_hash.as_bytes(),
        };

        let vote_hash = self.crypto.hash(protocol::Bytes::from(rlp::encode(&vote)));
        let hex_pubkeys = metadata
            .verifier_list
            .iter()
            .filter_map(|v| {
                if signed_voters.contains(&v.address.as_bytes()) {
                    Some(v.bls_pub_key.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        self.verify_proof_signature(
            ctx.clone(),
            block.header.height,
            vote_hash.clone(),
            proof.signature.clone(),
            hex_pubkeys,
        ).map_err(|e| {
            log::error!("[consensus] verify_proof_signature error, height {}, vote: {:?}, vote_hash:{:?}, sig:{:?}, signed_voter:{:?}",
            block.header.height,
            vote,
            vote_hash,
            proof.signature,
            signed_voters,
            );
            e
        })?;

        let weight_map = authority_list
            .iter()
            .map(|node| (node.address.clone(), node.vote_weight))
            .collect::<HashMap<overlord::types::Address, u32>>();

        self.verity_proof_weight(ctx.clone(), block.header.height, weight_map, signed_voters)?;

        Ok(())
    }

    fn verify_proof_signature(
        &self,
        _ctx: Context,
        block_height: u64,
        vote_hash: Bytes,
        aggregated_signature_bytes: Bytes,
        vote_keys: Vec<Hex>,
    ) -> ProtocolResult<()> {
        let mut pub_keys = Vec::new();
        for hex in vote_keys.into_iter() {
            pub_keys.push(convert_hex_to_bls_pubkeys(hex)?)
        }

        self.crypto
            .inner_verify_aggregated_signature(vote_hash, pub_keys, aggregated_signature_bytes)
            .map_err(|e| {
                log::error!("[consensus] verify_proof_signature error: {}", e);
                ConsensusError::VerifyProof(block_height, BlockProofField::Signature).into()
            })
    }

    fn verity_proof_weight(
        &self,
        _ctx: Context,
        block_height: u64,
        weight_map: HashMap<Bytes, u32>,
        signed_voters: Vec<Bytes>,
    ) -> ProtocolResult<()> {
        let total_validator_weight: u64 = weight_map.iter().map(|pair| u64::from(*pair.1)).sum();

        let mut accumulator = 0u64;
        for signed_voter_address in signed_voters {
            if weight_map.contains_key(signed_voter_address.as_ref()) {
                let weight = weight_map.get(signed_voter_address.as_ref()).ok_or({
                    log::error!(
                        "[consensus] verity_proof_weight,signed_voter_address: {:?}",
                        signed_voter_address
                    );
                    ConsensusError::VerifyProof(block_height, WeightNotFound)
                })?;
                accumulator += u64::from(*(weight));
            } else {
                log::error!(
                    "[consensus] verity_proof_weight,signed_voter_address: {:?}",
                    signed_voter_address
                );
                return Err(
                    ConsensusError::VerifyProof(block_height, BlockProofField::Validator).into(),
                );
            }
        }

        if 3 * accumulator <= 2 * total_validator_weight {
            log::error!(
                "[consensus] verity_proof_weight, accumulator: {}, total: {}",
                accumulator,
                total_validator_weight
            );

            return Err(ConsensusError::VerifyProof(block_height, BlockProofField::Weight).into());
        }
        Ok(())
    }
}

fn mock_crypto() -> OverlordCrypto {
    let priv_key = BlsPrivateKey::try_from(hex::decode("00000000000000000000000000000000d654c7a6747fc2e34808c1ebb1510bfb19b443d639f2fab6dc41fce9f634de37").unwrap().as_ref()).unwrap();
    OverlordCrypto::new(priv_key, HashMap::new(), "muta".into())
}

fn gen_remote_tx_hashmap(list: Vec<RichBlock>) -> SafeHashMap<Hash, SignedTransaction> {
    let mut remote_txs = HashMap::new();

    for rich_block in list.into_iter() {
        for tx in rich_block.txs {
            remote_txs.insert(tx.tx_hash.clone(), tx);
        }
    }

    Arc::new(RwLock::new(remote_txs))
}

fn gen_remote_block_hashmap(list: Vec<RichBlock>) -> SafeHashMap<u64, Block> {
    let mut remote_blocks = HashMap::new();
    for rich_block in list.into_iter() {
        remote_blocks.insert(rich_block.block.header.height, rich_block.block.clone());
    }

    Arc::new(RwLock::new(remote_blocks))
}

fn gen_remote_proof_hashmap(list: Vec<Proof>) -> SafeHashMap<u64, Proof> {
    let mut remote_proof = HashMap::new();
    for proof in list.into_iter() {
        remote_proof.insert(proof.height, proof.clone());
    }

    Arc::new(RwLock::new(remote_proof))
}

fn mock_chained_rich_block(len: u64, gap: u64, key_tool: &KeyTool) -> (Vec<RichBlock>, Vec<Proof>) {
    let mut list_rich_block = vec![];
    let mut list_proof = vec![];

    let genesis_rich_block = mock_genesis_rich_block();
    list_rich_block.push(genesis_rich_block.clone());
    // the proof of block 0 is n/a, we just stuff something here
    list_proof.push(genesis_rich_block.clone().block.header.proof);
    let mut last_rich_block = genesis_rich_block;

    let mut current_height = 1;

    let mut temp_rich_block: Vec<RichBlock> = vec![];

    let mut last_proof: Proof = Proof {
        height:     0,
        round:      0,
        block_hash: Hash::from_hex(
            "0x1122334455667788990011223344556677889900112233445566778899001122",
        )
        .unwrap(),
        signature:  Default::default(),
        bitmap:     Default::default(),
    };

    loop {
        let last_block_hash = Hash::digest(last_rich_block.block.header.encode_fixed().unwrap());
        let last_header = &last_rich_block.block.header;

        let txs = mock_tx_list(3, current_height);
        let tx_hashes: Vec<Hash> = txs.iter().map(|tx| tx.tx_hash.clone()).collect();
        let order_root = Merkle::from_hashes(tx_hashes.clone())
            .get_root_hash()
            .unwrap();
        let order_signed_transactions_hash = digest_signed_transactions(&txs).unwrap();

        let mut header = BlockHeader {
            chain_id: last_header.chain_id.clone(),
            height: current_height,
            exec_height: current_height,
            prev_hash: last_block_hash.clone(),
            timestamp: 0,
            order_root,
            order_signed_transactions_hash,
            logs_bloom: vec![],
            confirm_root: vec![],
            state_root: Hash::from_empty(),
            receipt_root: vec![],
            cycles_used: vec![],
            proposer: Address::from_hex("0x82c67c421d208fb7015d2da79550212a50f2e773").unwrap(),
            proof: last_proof,
            validator_version: 0,
            validators: vec![Validator {
                address:        Address::from_hex("0x82c67c421d208fb7015d2da79550212a50f2e773")
                    .unwrap(),
                propose_weight: 5,
                vote_weight:    5,
            }],
        };

        if last_header.height != 0 && current_height % gap == 0 {
            temp_rich_block.iter().for_each(|rich_block| {
                let height = rich_block.block.header.height;
                let confirm_root = rich_block.block.header.order_root.clone();
                let (exec_resp, receipt_root) = exec_txs(height, &rich_block.txs);

                header.exec_height = height;
                header.logs_bloom.push(exec_resp.logs_bloom);
                header.confirm_root.push(confirm_root);
                header.state_root = exec_resp.state_root;
                header.receipt_root.push(receipt_root);
                header.cycles_used.push(exec_resp.all_cycles_used);
            });

            temp_rich_block.clear();
        } else if last_header.height != 0 && header.height != 1 {
            header.exec_height -= temp_rich_block.len() as u64 + 1;
        } else if header.height == 1 {
            header.exec_height -= 1;
        }

        let block = Block {
            header,
            ordered_tx_hashes: tx_hashes,
        };

        let rich_block = RichBlock { block, txs };

        list_rich_block.push(rich_block.clone());
        temp_rich_block.push(rich_block.clone());
        last_rich_block = rich_block.clone();

        let current_block_hash = Hash::digest(rich_block.block.header.encode_fixed().unwrap());

        // generate proof for current height and for next block use
        last_proof = mock_proof(current_block_hash.clone(), current_height, 0, &key_tool);

        list_proof.push(last_proof.clone());

        current_height += 1;

        if current_height > len {
            break;
        }
    }

    (list_rich_block, list_proof)
}

fn mock_genesis_rich_block() -> RichBlock {
    let header = BlockHeader {
        chain_id:                       Hash::from_empty(),
        height:                         0,
        exec_height:                    0,
        prev_hash:                      Hash::from_empty(),
        timestamp:                      0,
        logs_bloom:                     vec![],
        order_root:                     Hash::from_empty(),
        order_signed_transactions_hash: Hash::from_empty(),
        confirm_root:                   vec![],
        state_root:                     Hash::from_empty(),
        receipt_root:                   vec![],
        cycles_used:                    vec![],
        proposer:                       Address::from_hex(
            "0x82c67c421d208fb7015d2da79550212a50f2e773",
        )
        .unwrap(),
        proof:                          Proof {
            height:     0,
            round:      0,
            block_hash: Hash::from_empty(),
            signature:  Bytes::new(),
            bitmap:     Bytes::new(),
        },
        validator_version:              0,
        validators:                     vec![Validator {
            address:        Address::from_hex("0x82c67c421d208fb7015d2da79550212a50f2e773")
                .unwrap(),
            propose_weight: 0,
            vote_weight:    0,
        }],
    };
    let genesis_block = Block {
        header,
        ordered_tx_hashes: vec![],
    };

    RichBlock {
        block: genesis_block,
        txs:   vec![],
    }
}

fn get_receipt(tx: &SignedTransaction, height: u64) -> Receipt {
    Receipt {
        state_root: MerkleRoot::from_empty(),
        height,
        tx_hash: tx.tx_hash.clone(),
        cycles_used: tx.raw.cycles_limit,
        events: vec![],
        response: ReceiptResponse {
            service_name: "sync".to_owned(),
            method:       "sync_exec".to_owned(),
            response:     ServiceResponse::<String> {
                code:          0,
                succeed_data:  "ok".to_owned(),
                error_message: "".to_owned(),
            },
        },
    }
}

// gen a lot of txs
fn mock_tx_list(num: usize, height: u64) -> Vec<SignedTransaction> {
    let mut txs = vec![];

    for i in 0..num {
        let raw = RawTransaction {
            chain_id:     Hash::from_empty(),
            nonce:        Hash::digest(Bytes::from(format!("{}", i))),
            timeout:      height,
            cycles_price: 1,
            cycles_limit: 1,
            request:      TransactionRequest {
                service_name: "test_service".to_owned(),
                method:       "test_method".to_owned(),
                payload:      "test_payload".to_owned(),
            },
        };

        let bytes = raw.encode_fixed().unwrap();

        // sign it vividly
        let hex_privkey =
            hex::decode("d654c7a6747fc2e34808c1ebb1510bfb19b443d639f2fab6dc41fce9f634de37")
                .unwrap();
        let test_privkey = Secp256k1PrivateKey::try_from(hex_privkey.as_ref()).unwrap();
        let test_pubkey = test_privkey.pub_key();
        let _test_address = Address::from_pubkey_bytes(test_pubkey.to_bytes()).unwrap();

        let tx_hash = Hash::digest(bytes);
        let hash_value = HashValue::try_from(tx_hash.as_bytes().as_ref())
            .ok()
            .unwrap();
        let signature = test_privkey.sign_message(&hash_value);

        let signed_tx = SignedTransaction {
            raw,
            tx_hash,
            pubkey: test_pubkey.to_bytes(),
            signature: signature.to_bytes(),
        };

        txs.push(signed_tx)
    }

    txs
}

// only the bls_private_key in KeyTool.overlordCrypto.private_key signs the
// Vote!!!!!!!
fn mock_proof(block_hash: Hash, height: u64, round: u64, key_tool: &KeyTool) -> Proof {
    let vote = Vote {
        height,
        round,
        vote_type: VoteType::Precommit,
        block_hash: block_hash.as_bytes(),
    };
    // println!("mocking proof, height: {}", height);
    // println!("      vote : {:?}", vote.clone());

    let vote_hash = key_tool.overlord_crypto.hash(Bytes::from(encode(&vote)));
    // println!("      vote_hash : {:?}", vote_hash);
    let bls_signature = key_tool.overlord_crypto.sign(vote_hash).unwrap();

    let signed_vote = SignedVote {
        voter:     key_tool.signer_node.secp_address.as_bytes(),
        signature: bls_signature,
        vote:      vote.clone(),
    };

    // println!("      signed_voter : {:?}", signed_vote.clone().voter);

    let signed_voter =
        vec![Address::from_hex("0x82c67c421d208fb7015d2da79550212a50f2e773").unwrap()]
            .iter()
            .cloned()
            .collect::<HashSet<Address>>(); //
    let mut bit_map = BitVec::from_elem(3, false);

    let mut authority_list: Vec<Node> = key_tool
        .verifier_list
        .clone()
        .iter()
        .map(|v| Node {
            address:        v.address.as_bytes(),
            propose_weight: v.propose_weight,
            vote_weight:    v.vote_weight,
        })
        .collect::<Vec<_>>();
    authority_list.sort();

    for (index, node) in authority_list.iter().enumerate() {
        if signed_voter.contains(&Address::from_bytes(node.address.clone()).unwrap()) {
            bit_map.set(index, true);
        }
    }

    let aggregated_signature = AggregatedSignature {
        signature:      key_tool
            .overlord_crypto
            .aggregate_signatures(vec![signed_vote.signature], vec![signed_vote.voter])
            .unwrap(),
        address_bitmap: Bytes::from(bit_map.to_bytes()),
    };

    // println!(
    //     "      address_bitmap : {:?}",
    //     aggregated_signature.clone().address_bitmap
    // );
    //
    // println!(
    //     "       aggregated_signature.signature_bytes : {:?}",
    //     aggregated_signature.clone().signature
    // );

    let aggregated_vote = AggregatedVote {
        signature: aggregated_signature,

        vote_type: vote.vote_type,
        height,
        round,
        block_hash: block_hash.as_bytes(),
        leader: key_tool.signer_node.secp_address.as_bytes(),
    };

    Proof {
        height:     aggregated_vote.height,
        round:      0,
        block_hash: Hash::from_bytes(aggregated_vote.block_hash).unwrap(),
        signature:  aggregated_vote.signature.signature.clone(),
        bitmap:     aggregated_vote.signature.address_bitmap.clone(),
    }
}

fn exec_txs(height: u64, txs: &[SignedTransaction]) -> (ExecutorResp, MerkleRoot) {
    let mut receipts = vec![];
    let mut all_cycles_used = 0;

    for tx in txs.iter() {
        let receipt = get_receipt(tx, height);
        all_cycles_used += receipt.cycles_used;
        receipts.push(receipt);
    }
    let receipt_root = Merkle::from_hashes(
        receipts
            .iter()
            .map(|r| Hash::digest(r.to_owned().encode_fixed().unwrap()))
            .collect::<Vec<_>>(),
    )
    .get_root_hash()
    .unwrap_or_else(Hash::from_empty);

    (
        ExecutorResp {
            receipts,
            all_cycles_used,
            logs_bloom: Default::default(),
            state_root: MerkleRoot::from_empty(),
        },
        receipt_root,
    )
}

#[derive(Clone)]
struct SignerNode {
    secp_private_key: Secp256k1PrivateKey,
    secp_public_key:  Secp256k1PublicKey,
    secp_address:     Address,
}

impl SignerNode {
    pub fn new(
        secp_private_key: Secp256k1PrivateKey,
        secp_public_key: Secp256k1PublicKey,
        secp_address: Address,
    ) -> Self {
        SignerNode {
            secp_private_key,
            secp_public_key,
            secp_address,
        }
    }
}

struct KeyTool {
    signer_node:     SignerNode,
    overlord_crypto: Arc<OverlordCrypto>,
    verifier_list:   Vec<ValidatorExtend>,
}

impl KeyTool {
    pub fn new(
        signer_node: SignerNode,
        overlord_crypto: Arc<OverlordCrypto>,
        verifier_list: Vec<ValidatorExtend>,
    ) -> Self {
        KeyTool {
            signer_node,
            overlord_crypto,
            verifier_list,
        }
    }
}

fn get_mock_key_tool() -> KeyTool {
    let hex_privkey =
        hex::decode("d654c7a6747fc2e34808c1ebb1510bfb19b443d639f2fab6dc41fce9f634de37").unwrap();
    let secp_privkey = Secp256k1PrivateKey::try_from(hex_privkey.as_ref()).unwrap();
    let secp_pubkey: Secp256k1PublicKey = secp_privkey.pub_key();
    let secp_address = Address::from_pubkey_bytes(secp_pubkey.to_bytes()).unwrap();

    let signer_node = SignerNode::new(secp_privkey, secp_pubkey, secp_address);

    // generate BLS/OverlordCrypto
    let mut bls_priv_key = Vec::new();
    bls_priv_key.extend_from_slice(&[0u8; 16]);
    let mut tmp =
        hex::decode("d654c7a6747fc2e34808c1ebb1510bfb19b443d639f2fab6dc41fce9f634de37").unwrap();
    bls_priv_key.append(&mut tmp);
    let bls_priv_key = BlsPrivateKey::try_from(bls_priv_key.as_ref()).unwrap();

    let (bls_pub_keys, common_ref) = get_mock_public_keys_and_common_ref();

    let mock_crypto = OverlordCrypto::new(bls_priv_key, bls_pub_keys, common_ref);

    KeyTool::new(signer_node, Arc::new(mock_crypto), mock_verifier_list())
}

fn get_mock_public_keys_and_common_ref() -> (HashMap<Bytes, BlsPublicKey>, BlsCommonReference) {
    let mut bls_pub_keys: HashMap<Bytes, BlsPublicKey> = HashMap::new();

    // weight = 3
    let bls_hex = Hex::from_string("0x0403142cf2dc63d122cc31e8245daa661b4b7c47793a9ab14e3c27430e3a835cb50b0bda0ea90480765d73d509e02c15f8031c20a77254fb0a8ec2919f2ed13b02034153776ad30d8fad90da15e0b85cd98cb81fa5f810c62563c8b507ef11604e".to_string()
    ).unwrap();
    let bls_hex = hex::decode(bls_hex.as_string_trim0x()).unwrap();
    bls_pub_keys.insert(
        Address::from_hex("0x82c67c421d208fb7015d2da79550212a50f2e773")
            .unwrap()
            .as_bytes(),
        BlsPublicKey::try_from(bls_hex.as_ref()).unwrap(),
    );

    // weight = 1
    let bls_hex = Hex::from_string("0x0414a4665f0d3d0a2b034e933a40f8e84a2113b12cdc33c1f17d28a4a5313768cfdb5c1b6d8f9a3e0df54c87d6fb196b1a0e93d284bfe15814f5bce36c4092bdf26c88b77798570a3ac251c630cc7995a89047f51b2a9aebb1046d81d52486be32".to_string()
    ).unwrap();
    let bls_hex = hex::decode(bls_hex.as_string_trim0x()).unwrap();
    bls_pub_keys.insert(
        Address::from_hex("0x6c9e6d3ccf42a3e67f6bf132a53a92db3bc065b5")
            .unwrap()
            .as_bytes(),
        BlsPublicKey::try_from(bls_hex.as_ref()).unwrap(),
    );

    // weight = 1
    let bls_hex = Hex::from_string("0x04051326c12edd4eded8a215566390a03896389b80422a652327fc438d64d65f1f60065b69215bb5c0864a46e149ba30d21138ad5981ad6e0c79a357eeb686a4d0cd690caccca169e6393a9b1cc850c203f474276ae71c291a1523ce76f0a8851e".to_string()
    ).unwrap();
    let bls_hex = hex::decode(bls_hex.as_string_trim0x()).unwrap();
    bls_pub_keys.insert(
        Address::from_hex("0x9b80c81780b782d03b3cebe0033bb78cb8d855d7")
            .unwrap()
            .as_bytes(),
        BlsPublicKey::try_from(bls_hex.as_ref()).unwrap(),
    );

    let hex_common_ref = hex::decode("3453376d613471795964").unwrap();
    let common_ref: BlsCommonReference =
        std::str::from_utf8(hex_common_ref.as_ref()).unwrap().into();

    (bls_pub_keys, common_ref)
}

fn mock_verifier_list() -> Vec<ValidatorExtend> {
    vec![
        ValidatorExtend {

            bls_pub_key: Hex::from_string("0x0403142cf2dc63d122cc31e8245daa661b4b7c47793a9ab14e3c27430e3a835cb50b0bda0ea90480765d73d509e02c15f8031c20a77254fb0a8ec2919f2ed13b02034153776ad30d8fad90da15e0b85cd98cb81fa5f810c62563c8b507ef11604e".to_owned()).unwrap(),
            address:        Address::from_hex("0x82c67c421d208fb7015d2da79550212a50f2e773").unwrap(),
            propose_weight: 5,
            vote_weight:    5,
        },
        ValidatorExtend {
            bls_pub_key: Hex::from_string("0x0414a4665f0d3d0a2b034e933a40f8e84a2113b12cdc33c1f17d28a4a5313768cfdb5c1b6d8f9a3e0df54c87d6fb196b1a0e93d284bfe15814f5bce36c4092bdf26c88b77798570a3ac251c630cc7995a89047f51b2a9aebb1046d81d52486be32".to_owned()).unwrap(),
            address:        Address::from_hex("0x6c9e6d3ccf42a3e67f6bf132a53a92db3bc065b5").unwrap(),
            propose_weight: 1,
            vote_weight:    1,
        },
        ValidatorExtend {
            bls_pub_key: Hex::from_string("0x04051326c12edd4eded8a215566390a03896389b80422a652327fc438d64d65f1f60065b69215bb5c0864a46e149ba30d21138ad5981ad6e0c79a357eeb686a4d0cd690caccca169e6393a9b1cc850c203f474276ae71c291a1523ce76f0a8851e".to_owned()).unwrap(),
            address:        Address::from_hex("0x9b80c81780b782d03b3cebe0033bb78cb8d855d7").unwrap(),
            propose_weight: 1,
            vote_weight:    1,
        },
    ]
}

// {
// "common_ref": "3453376d613471795964",
// "keypairs": [
// {
// "index": 1,
// "private_key":
// "d654c7a6747fc2e34808c1ebb1510bfb19b443d639f2fab6dc41fce9f634de37",
// "public_key":
// "03eacfbe0c216b9afd1370da335a49f49b88a4a34cc99c15885812e36bfab774fd",
// "address": "82c67c421d208fb7015d2da79550212a50f2e773",
// "bls_public_key":
// "0403142cf2dc63d122cc31e8245daa661b4b7c47793a9ab14e3c27430e3a835cb50b0bda0ea90480765d73d509e02c15f8031c20a77254fb0a8ec2919f2ed13b02034153776ad30d8fad90da15e0b85cd98cb81fa5f810c62563c8b507ef11604e"
// },
// {
// "index": 2,
// "private_key":
// "aa0374de198a4ba1187e4a41b316e159ab256ce129bcc76c10d746edfe6f82e5",
// "public_key":
// "03bf246a94a98a4c96a71a02187b8bc98539f853287800a7fa38d67b2ed9643bd2",
// "address": "6c9e6d3ccf42a3e67f6bf132a53a92db3bc065b5",
// "bls_public_key":
// "0414a4665f0d3d0a2b034e933a40f8e84a2113b12cdc33c1f17d28a4a5313768cfdb5c1b6d8f9a3e0df54c87d6fb196b1a0e93d284bfe15814f5bce36c4092bdf26c88b77798570a3ac251c630cc7995a89047f51b2a9aebb1046d81d52486be32"
// },
// {
// "index": 3,
// "private_key":
// "0c27b03412cae1df9cd7374c2eab1daff1023721f9930f44d7f5a0f8172a2b32",
// "public_key":
// "028b1e157e441a3cd6f2f6116ecac24235c02e1d66471407484fba5dca44242a8f",
// "address": "9b80c81780b782d03b3cebe0033bb78cb8d855d7",
// "bls_public_key":
// "04051326c12edd4eded8a215566390a03896389b80422a652327fc438d64d65f1f60065b69215bb5c0864a46e149ba30d21138ad5981ad6e0c79a357eeb686a4d0cd690caccca169e6393a9b1cc850c203f474276ae71c291a1523ce76f0a8851e"
// },
// {
// "index": 4,
// "private_key":
// "fc57fc08dc883891b88292a2d2915050b9a09c8a6096dfb04372bdd15162665e",
// "public_key":
// "03c00ce1cb83fbf8151c98d110d8cc4d83a6884fcee27916d416882ce42ae5925c",
// "address": "0804d72af82a05733df54a72278a73629b4bea17",
// "bls_public_key":
// "040d606915df9b432acf66cdd9338d240fe781fb05249957201613113dda384960022649e9fd9e8b66c8962bb5e223d30405c5590e8d9cae893384ace1789d61a73b325e638ef5ee1c47d3b2a5a863e19b54fbfc881b5cc0b8bb05fce04d514807"
// }
// ]
// }

fn assert_sync(status: CurrentConsensusStatus, latest_block: Block) {
    let exec_gap = latest_block.header.height - latest_block.header.exec_height;

    assert_eq!(status.latest_committed_height, latest_block.header.height);
    assert_eq!(status.exec_height, latest_block.header.height);
    assert_eq!(status.current_proof.height, status.latest_committed_height);
    assert_eq!(status.list_confirm_root.len(), exec_gap as usize);
    assert_eq!(status.list_cycles_used.len(), exec_gap as usize);
    assert_eq!(status.list_logs_bloom.len(), exec_gap as usize);
    assert_eq!(status.list_receipt_root.len(), exec_gap as usize);
}
