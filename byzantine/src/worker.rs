use std::convert::TryFrom;
use std::sync::Arc;

use bytes::Bytes;
use futures::{channel::mpsc::UnboundedReceiver, stream::StreamExt};
use lazy_static::lazy_static;
use overlord::types::{
    AggregatedVote, Choke, PoLC, Proposal, SignedChoke, SignedProposal, SignedVote, Vote, VoteType,
};
use rlp::Encodable;

use common_crypto::Secp256k1PrivateKey;
use core_consensus::fixed_types::FixedPill;
use core_consensus::message::{
    BROADCAST_HEIGHT, END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_CHOKE,
    END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE,
};
use core_consensus::util::OverlordCrypto;
use core_mempool::{
    MsgNewTxs, MsgPullTxs, MsgPushTxs, END_GOSSIP_NEW_TXS, RPC_PULL_TXS, RPC_RESP_PULL_TXS,
};
use core_network::{PeerId, PeerIdExt};
use protocol::traits::{Context, Gossip, MessageCodec, PeerTrust, Priority, Rpc};
use protocol::types::{
    Address, Hash, Hex, MerkleRoot, Metadata, Proof, SignedTransaction, Validator,
};

use crate::behaviors::{
    Behavior, MessageType, NewChoke, NewProposal, NewQC, NewTx, NewVote, PullTxs, Request,
};
use crate::invalid_types::{
    gen_invalid_address_new_choke, gen_invalid_block_hash_new_proposal,
    gen_invalid_block_hash_new_qc, gen_invalid_block_hash_new_vote,
    gen_invalid_block_proposer_new_proposal, gen_invalid_chain_id_new_proposal,
    gen_invalid_chain_id_signed_tx, gen_invalid_confirm_root_new_proposal,
    gen_invalid_content_struct_new_proposal, gen_invalid_cycle_used_new_proposal,
    gen_invalid_cycles_limit_signed_tx, gen_invalid_cycles_price_signed_tx,
    gen_invalid_exec_height_new_proposal, gen_invalid_from_new_vote, gen_invalid_hash_pull_txs,
    gen_invalid_hash_signed_tx, gen_invalid_height_new_choke, gen_invalid_height_new_proposal,
    gen_invalid_height_new_qc, gen_invalid_height_new_vote, gen_invalid_height_pull_txs,
    gen_invalid_leader_new_qc, gen_invalid_lock_new_proposal, gen_invalid_nonce_dup_signed_tx,
    gen_invalid_nonce_of_rand_len_signed_tx, gen_invalid_order_root_new_proposal,
    gen_invalid_prev_hash_new_proposal, gen_invalid_proof_new_proposal,
    gen_invalid_prop_height_new_proposal, gen_invalid_prop_proposer_new_proposal,
    gen_invalid_receipt_root_new_proposal, gen_invalid_request_signed_tx,
    gen_invalid_round_new_choke, gen_invalid_round_new_proposal, gen_invalid_round_new_qc,
    gen_invalid_round_new_vote, gen_invalid_sender_signed_tx, gen_invalid_sig_new_choke,
    gen_invalid_sig_new_proposal, gen_invalid_sig_new_qc, gen_invalid_sig_new_vote,
    gen_invalid_sig_signed_tx, gen_invalid_signed_tx_hash_new_proposal,
    gen_invalid_state_root_new_proposal, gen_invalid_struct_new_choke,
    gen_invalid_struct_new_proposal, gen_invalid_struct_new_qc, gen_invalid_struct_new_vote,
    gen_invalid_timeout_signed_tx, gen_invalid_timestamp_new_proposal,
    gen_invalid_tx_hash_new_proposal, gen_invalid_validators_new_proposal,
    gen_invalid_version_new_proposal, gen_invalid_voter_new_vote, gen_not_exists_txs_pull_txs,
    gen_valid_new_proposal, InvalidStruct,
};
use crate::utils::{
    gen_positive_range, gen_random_bytes, gen_valid_signed_choke, gen_valid_signed_vote, time_now,
};

lazy_static! {
    static ref TEST_PRI_KEY: Secp256k1PrivateKey = {
        let hex_prikey = Hex::from_string(
            "0x5ec982173d54d830b6789cbbbe43eaa2853a5ff752d1ebc1b266cf9790314f8a".to_string(),
        )
        .unwrap();
        Secp256k1PrivateKey::try_from(hex_prikey.decode().as_ref())
            .expect("get test pri_key failed")
    };
}

macro_rules! send_new_tx {
    ($self_: ident, $ctx: ident, $behavior: ident, $func: ident) => {{
        let behavior = $behavior.clone();
        let metadata = $self_.metadata.clone();
        let height = $self_.state.height;
        let network = Arc::<_>::clone(&$self_.network);
        tokio::spawn(async move {
            let batch_stxs: Vec<SignedTransaction> = (0..behavior.msg_num)
                .map(|_| $func(&TEST_PRI_KEY, height, &metadata))
                .collect();
            let gossip_txs = MsgNewTxs { batch_stxs };
            send(&network, gossip_txs, $ctx, END_GOSSIP_NEW_TXS, &behavior).await;
        });
    }};
}

macro_rules! send_push_txs {
    ($self_: ident, $ctx: ident, $behavior: ident, $func: ident) => {{
        let behavior = $behavior.clone();
        let metadata = $self_.metadata.clone();
        let height = $self_.state.height;
        let network = Arc::<_>::clone(&$self_.network);
        tokio::spawn(async move {
            let batch_stxs: Vec<SignedTransaction> = (0..behavior.msg_num)
                .map(|_| $func(&TEST_PRI_KEY, height, &metadata))
                .collect();
            let push_txs = MsgPushTxs {
                sig_txs: batch_stxs,
            };
            let _ = network
                .response::<MsgPushTxs>($ctx, RPC_RESP_PULL_TXS, Ok(push_txs), behavior.priority)
                .await;
        });
    }};
}

macro_rules! send_pull_txs {
    ($self_: ident, $ctx: ident, $behavior: ident, $func: ident) => {{
        let behavior = $behavior.clone();
        let height = $self_.state.height;
        let network = Arc::<_>::clone(&$self_.network);
        tokio::spawn(async move {
            for _ in (0..behavior.msg_num) {
                let pull_msg = $func(height);
                let _ = network
                    .call::<MsgPullTxs, MsgPushTxs>(
                        $ctx.clone(),
                        RPC_PULL_TXS,
                        pull_msg,
                        behavior.priority.clone(),
                    )
                    .await;
            }
        });
    }};
}

macro_rules! send_new_proposal {
    ($self_: ident, $ctx: ident, $behavior: ident, $func: ident) => {{
        let behavior = $behavior.clone();
        let state = $self_.state.clone();
        let metadata = $self_.metadata.clone();
        let crypto = $self_.crypto.clone();
        let address = $self_.address.clone();
        let pub_key = $self_.pub_key.clone();
        let validators = $self_.validators.clone();
        let network = Arc::<_>::clone(&$self_.network);
        tokio::spawn(async move {
            let messages: Vec<Vec<u8>> = (0..behavior.msg_num)
                .map(|_| $func(&state, &metadata, &crypto, &address, &pub_key, &validators))
                .collect();
            for msg in messages {
                send(
                    &network,
                    msg,
                    $ctx.clone(),
                    END_GOSSIP_SIGNED_PROPOSAL,
                    &behavior,
                )
                .await;
            }
        });
    }};
}

macro_rules! send_new_vote_or_choke {
    ($self_: ident, $ctx: ident, $behavior: ident, $func: ident, $end: ident) => {{
        let behavior = $behavior.clone();
        let state = $self_.state.clone();
        let crypto = $self_.crypto.clone();
        let pub_key = $self_.pub_key.clone();
        let network = Arc::<_>::clone(&$self_.network);
        tokio::spawn(async move {
            let messages: Vec<Vec<u8>> = (0..behavior.msg_num)
                .map(|_| $func(&state, &crypto, &pub_key))
                .collect();
            for msg in messages {
                send(&network, msg, $ctx.clone(), $end, &behavior).await;
            }
        });
    }};
}

macro_rules! send_new_vote {
    ($self_: ident, $ctx: ident, $behavior: ident, $func: ident) => {
        send_new_vote_or_choke!($self_, $ctx, $behavior, $func, END_GOSSIP_SIGNED_VOTE);
    };
}

macro_rules! send_new_choke {
    ($self_: ident, $ctx: ident, $behavior: ident, $func: ident) => {
        send_new_vote_or_choke!($self_, $ctx, $behavior, $func, END_GOSSIP_SIGNED_CHOKE);
    };
}

macro_rules! send_new_qc {
    ($self_: ident, $ctx: ident, $behavior: ident, $func: ident) => {{
        let behavior = $behavior.clone();
        let state = $self_.state.clone();
        let pub_key = $self_.pub_key.clone();
        let network = Arc::<_>::clone(&$self_.network);
        tokio::spawn(async move {
            let messages: Vec<Vec<u8>> = (0..behavior.msg_num)
                .map(|_| $func(&state, &pub_key))
                .collect();
            for msg in messages {
                send(
                    &network,
                    msg,
                    $ctx.clone(),
                    END_GOSSIP_AGGREGATED_VOTE,
                    &behavior,
                )
                .await;
            }
        });
    }};
}

#[derive(Clone, Debug)]
pub struct State {
    pub height:         u64,
    pub round:          u64,
    pub exec_height:    u64,
    pub prev_hash:      Hash,
    pub prev_timestamp: u64,
    pub state_root:     MerkleRoot,
    pub confirm_root:   Vec<MerkleRoot>,
    pub receipt_root:   Vec<MerkleRoot>,
    pub cycles_used:    Vec<u64>,
    pub lock:           Option<PoLC>,
    pub proof:          Proof,
}

impl Default for State {
    fn default() -> Self {
        State {
            height:         0,
            round:          0,
            exec_height:    0,
            prev_hash:      Hash::from_empty(),
            prev_timestamp: time_now(),
            state_root:     MerkleRoot::from_empty(),
            confirm_root:   vec![],
            receipt_root:   vec![],
            cycles_used:    vec![],
            lock:           None,
            proof:          Proof {
                height:     0,
                round:      0,
                block_hash: Hash::from_empty(),
                signature:  Bytes::new(),
                bitmap:     Bytes::new(),
            },
        }
    }
}

pub struct Worker<N: Rpc + PeerTrust + Gossip + 'static> {
    state:      State,
    address:    Address,
    pub_key:    Bytes,
    metadata:   Metadata,
    validators: Vec<Validator>,
    crypto:     Arc<OverlordCrypto>,
    network:    Arc<N>,

    from_timeout: UnboundedReceiver<(Context, Vec<Behavior>)>,
}

impl<N> Worker<N>
where
    N: Rpc + PeerTrust + Gossip + 'static,
{
    pub fn new(
        address: Address,
        pub_key: Bytes,
        metadata: Metadata,
        validators: Vec<Validator>,
        crypto: OverlordCrypto,
        network: Arc<N>,
        from_timeout: UnboundedReceiver<(Context, Vec<Behavior>)>,
    ) -> Worker<N> {
        Worker {
            state: State::default(),
            address,
            pub_key,
            crypto: Arc::new(crypto),
            metadata,
            validators,
            network,
            from_timeout,
        }
    }

    pub async fn run(mut self) {
        let mut cnt = 0;
        loop {
            let (ctx, behaviors) = self.from_timeout.next().await.expect("Channel is down!");
            for behavior in behaviors {
                cnt += 1;
                println!(
                    "[h: {}, r: {}] worker process {:?}, accumulative process {} behaviors",
                    self.state.height, self.state.round, behavior.msg_type, cnt
                );
                self.process(ctx.clone(), &behavior).await;
            }
        }
    }

    pub async fn process(&mut self, ctx: Context, behavior: &Behavior) {
        match &behavior.msg_type {
            MessageType::NewTxs(new_tx) => match new_tx {
                NewTx::InvalidStruct => self.send_invalid_struct_of_new_tx(ctx, behavior).await,
                NewTx::InvalidHash => send_new_tx!(self, ctx, behavior, gen_invalid_hash_signed_tx),
                NewTx::InvalidSig => send_new_tx!(self, ctx, behavior, gen_invalid_sig_signed_tx),
                NewTx::InvalidChainID => {
                    send_new_tx!(self, ctx, behavior, gen_invalid_chain_id_signed_tx)
                }
                NewTx::InvalidCyclesPrice => {
                    send_new_tx!(self, ctx, behavior, gen_invalid_cycles_price_signed_tx)
                }
                NewTx::InvalidCyclesLimit => {
                    send_new_tx!(self, ctx, behavior, gen_invalid_cycles_limit_signed_tx)
                }
                NewTx::InvalidNonceOfRandLen => {
                    send_new_tx!(self, ctx, behavior, gen_invalid_nonce_of_rand_len_signed_tx)
                }
                NewTx::InvalidNonceDup => {
                    self.send_invalid_nonce_dup_of_new_tx(ctx, behavior).await
                }
                NewTx::InvalidRequest => {
                    send_new_tx!(self, ctx, behavior, gen_invalid_request_signed_tx)
                }
                NewTx::InvalidTimeout => {
                    send_new_tx!(self, ctx, behavior, gen_invalid_timeout_signed_tx)
                }
                NewTx::InvalidSender => {
                    send_new_tx!(self, ctx, behavior, gen_invalid_sender_signed_tx)
                }
            },
            MessageType::RecvProposal(pull_txs) => match pull_txs {
                PullTxs::Valid => self.set_state(behavior.request.as_ref()).await,
                PullTxs::InvalidHeight => {
                    send_pull_txs!(self, ctx, behavior, gen_invalid_height_pull_txs)
                }
                PullTxs::InvalidHash => {
                    send_pull_txs!(self, ctx, behavior, gen_invalid_hash_pull_txs)
                }
                PullTxs::NotExistTxs => {
                    send_pull_txs!(self, ctx, behavior, gen_not_exists_txs_pull_txs)
                }
                PullTxs::InvalidStruct => self.send_invalid_struct_of_pull_txs(ctx, behavior).await,
            },
            MessageType::SendProposal(new_proposal) => match new_proposal {
                NewProposal::Valid => {
                    send_new_proposal!(self, ctx, behavior, gen_valid_new_proposal)
                }
                NewProposal::InvalidStruct => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_struct_new_proposal)
                }
                NewProposal::InvalidChainId => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_chain_id_new_proposal)
                }
                NewProposal::InvalidPrevHash => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_prev_hash_new_proposal)
                }
                NewProposal::InvalidHeight => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_height_new_proposal)
                }
                NewProposal::InvalidExecHeight => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_exec_height_new_proposal)
                }
                NewProposal::InvalidTimestamp => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_timestamp_new_proposal)
                }
                NewProposal::InvalidOrderRoot => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_order_root_new_proposal)
                }
                NewProposal::InvalidSignedTxsHash => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_signed_tx_hash_new_proposal)
                }
                NewProposal::InvalidConfirmRoot => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_confirm_root_new_proposal)
                }
                NewProposal::InvalidStateRoot => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_state_root_new_proposal)
                }
                NewProposal::InvalidReceiptRoot => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_receipt_root_new_proposal)
                }
                NewProposal::InvalidCyclesUsed => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_cycle_used_new_proposal)
                }
                NewProposal::InvalidBlockProposer => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_block_proposer_new_proposal)
                }
                NewProposal::InvalidProof => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_proof_new_proposal)
                }
                NewProposal::InvalidVersion => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_version_new_proposal)
                }
                NewProposal::InvalidValidators => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_validators_new_proposal)
                }
                NewProposal::InvalidTxHash => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_tx_hash_new_proposal)
                }
                NewProposal::InvalidSig => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_sig_new_proposal)
                }
                NewProposal::InvalidProposalHeight => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_prop_height_new_proposal)
                }
                NewProposal::InvalidRound => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_round_new_proposal)
                }
                NewProposal::InvalidContentStruct => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_content_struct_new_proposal)
                }
                NewProposal::InvalidBlockHash => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_block_hash_new_proposal)
                }
                NewProposal::InvalidLock => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_lock_new_proposal)
                }
                NewProposal::InvalidProposalProposer => {
                    send_new_proposal!(self, ctx, behavior, gen_invalid_prop_proposer_new_proposal)
                }
            },
            MessageType::SendVote(new_vote) => match new_vote {
                NewVote::InvalidStruct => {
                    send_new_vote!(self, ctx, behavior, gen_invalid_struct_new_vote)
                }
                NewVote::InvalidHeight => {
                    send_new_vote!(self, ctx, behavior, gen_invalid_height_new_vote)
                }
                NewVote::InvalidRound => {
                    send_new_vote!(self, ctx, behavior, gen_invalid_round_new_vote)
                }
                NewVote::InvalidBlockHash => {
                    send_new_vote!(self, ctx, behavior, gen_invalid_block_hash_new_vote)
                }
                NewVote::InvalidSig => {
                    send_new_vote!(self, ctx, behavior, gen_invalid_sig_new_vote)
                }
                NewVote::InvalidVoter => {
                    send_new_vote!(self, ctx, behavior, gen_invalid_voter_new_vote)
                }
            },
            MessageType::SendQC(new_qc) => match new_qc {
                NewQC::InvalidStruct => {
                    send_new_qc!(self, ctx, behavior, gen_invalid_struct_new_qc)
                }
                NewQC::InvalidHeight => {
                    send_new_qc!(self, ctx, behavior, gen_invalid_height_new_qc)
                }
                NewQC::InvalidRound => send_new_qc!(self, ctx, behavior, gen_invalid_round_new_qc),
                NewQC::InvalidBlockHash => {
                    send_new_qc!(self, ctx, behavior, gen_invalid_block_hash_new_qc)
                }
                NewQC::InvalidSig => send_new_qc!(self, ctx, behavior, gen_invalid_sig_new_qc),
                NewQC::InvalidLeader => {
                    send_new_qc!(self, ctx, behavior, gen_invalid_leader_new_qc)
                }
            },
            MessageType::SendChoke(new_choke) => match new_choke {
                NewChoke::InvalidStruct => {
                    send_new_choke!(self, ctx, behavior, gen_invalid_struct_new_choke)
                }
                NewChoke::InvalidHeight => {
                    send_new_choke!(self, ctx, behavior, gen_invalid_height_new_choke)
                }
                NewChoke::InvalidRound => {
                    send_new_choke!(self, ctx, behavior, gen_invalid_round_new_choke)
                }
                NewChoke::InvalidFrom => {
                    send_new_choke!(self, ctx, behavior, gen_invalid_from_new_vote)
                }
                NewChoke::InvalidSig => {
                    send_new_choke!(self, ctx, behavior, gen_invalid_sig_new_choke)
                }
                NewChoke::InvalidAddress => {
                    send_new_choke!(self, ctx, behavior, gen_invalid_address_new_choke)
                }
            },
            MessageType::SendHeight => self.send_invalid_new_height(ctx, behavior).await,
            MessageType::PullTxs(new_tx) => match new_tx {
                NewTx::InvalidStruct => self.send_invalid_struct_of_push_txs(ctx, behavior).await,
                NewTx::InvalidHash => {
                    send_push_txs!(self, ctx, behavior, gen_invalid_hash_signed_tx)
                }
                NewTx::InvalidSig => send_push_txs!(self, ctx, behavior, gen_invalid_sig_signed_tx),
                NewTx::InvalidChainID => {
                    send_push_txs!(self, ctx, behavior, gen_invalid_chain_id_signed_tx)
                }
                NewTx::InvalidCyclesPrice => {
                    send_push_txs!(self, ctx, behavior, gen_invalid_cycles_price_signed_tx)
                }
                NewTx::InvalidCyclesLimit => {
                    send_push_txs!(self, ctx, behavior, gen_invalid_cycles_limit_signed_tx)
                }
                NewTx::InvalidNonceOfRandLen => {
                    send_push_txs!(self, ctx, behavior, gen_invalid_nonce_of_rand_len_signed_tx)
                }
                NewTx::InvalidRequest => {
                    send_push_txs!(self, ctx, behavior, gen_invalid_request_signed_tx)
                }
                NewTx::InvalidTimeout => {
                    send_push_txs!(self, ctx, behavior, gen_invalid_timeout_signed_tx)
                }
                NewTx::InvalidSender => {
                    send_push_txs!(self, ctx, behavior, gen_invalid_sender_signed_tx)
                }
                _ => panic!("not support yet!"),
            },
            MessageType::RecvQC
            | MessageType::RecvVote
            | MessageType::RecvChoke
            | MessageType::RecvHeight => self.set_state(behavior.request.as_ref()).await,
        }
    }

    pub async fn send_invalid_new_height(&mut self, ctx: Context, behavior: &Behavior) {
        let behavior = behavior.clone();
        let height = self.state.height;
        let network = Arc::<_>::clone(&self.network);
        tokio::spawn(async move {
            let messages: Vec<u64> = (0..behavior.msg_num)
                .map(|_| gen_positive_range(height, 20))
                .collect();
            for msg in messages {
                send(&network, msg, ctx.clone(), BROADCAST_HEIGHT, &behavior).await;
            }
        });
    }

    pub async fn send_invalid_struct_of_pull_txs(&mut self, ctx: Context, behavior: &Behavior) {
        let behavior = behavior.clone();
        let network = Arc::<_>::clone(&self.network);
        tokio::spawn(async move {
            for _ in 0..behavior.msg_num {
                let pull_msg = InvalidStruct::gen(100);
                let _ = network
                    .call::<InvalidStruct, MsgPushTxs>(
                        ctx.clone(),
                        RPC_PULL_TXS,
                        pull_msg,
                        behavior.priority,
                    )
                    .await;
            }
        });
    }

    pub async fn send_invalid_struct_of_new_tx(&self, ctx: Context, behavior: &Behavior) {
        let behavior = behavior.clone();
        let network = Arc::<_>::clone(&self.network);
        tokio::spawn(async move {
            let messages: Vec<InvalidStruct> = (0..behavior.msg_num)
                .map(|_| InvalidStruct::gen(1000))
                .collect();
            for msg in messages {
                send(&network, msg, ctx.clone(), END_GOSSIP_NEW_TXS, &behavior).await;
            }
        });
    }

    pub async fn send_invalid_struct_of_push_txs(&self, ctx: Context, behavior: &Behavior) {
        let behavior = behavior.clone();
        let network = Arc::<_>::clone(&self.network);
        tokio::spawn(async move {
            let messages: Vec<InvalidStruct> = (0..behavior.msg_num)
                .map(|_| InvalidStruct::gen(1000))
                .collect();
            for msg in messages {
                let _ = network
                    .response::<InvalidStruct>(
                        ctx.clone(),
                        RPC_RESP_PULL_TXS,
                        Ok(msg),
                        behavior.priority,
                    )
                    .await;
            }
        });
    }

    pub async fn send_invalid_nonce_dup_of_new_tx(&self, ctx: Context, behavior: &Behavior) {
        let nonce = Hash::digest(gen_random_bytes(20));
        let behavior = behavior.clone();
        let metadata = self.metadata.clone();
        let height = self.state.height;
        let network = Arc::<_>::clone(&self.network);
        tokio::spawn(async move {
            let batch_stxs: Vec<SignedTransaction> = (0..behavior.msg_num)
                .map(|_| {
                    gen_invalid_nonce_dup_signed_tx(&TEST_PRI_KEY, height, &metadata, nonce.clone())
                })
                .collect();
            let gossip_txs = MsgNewTxs { batch_stxs };
            send(&network, gossip_txs, ctx, END_GOSSIP_NEW_TXS, &behavior).await;
        });
    }

    async fn set_state(&mut self, req_opt: Option<&Request>) {
        if let Some(req) = req_opt {
            match req {
                Request::RecvProposal(proposal) => {
                    let signed_proposal: SignedProposal<FixedPill> =
                        rlp::decode(&proposal.0).expect("decode signed_proposal failed");
                    let proposal = signed_proposal.proposal;
                    if proposal.height > self.state.height
                        || (proposal.height == self.state.height
                            && proposal.round >= self.state.round)
                    {
                        let header = proposal.content.inner.block.header.clone();
                        self.state.height = proposal.height;
                        self.state.round = proposal.round;
                        self.state.prev_hash = header.prev_hash;
                        self.state.proof = header.proof;
                        self.state.state_root = header.state_root;
                        self.state.exec_height = header.exec_height;
                        self.state.confirm_root = header.confirm_root;
                        self.state.receipt_root = header.receipt_root;
                        self.state.cycles_used = header.cycles_used;
                        self.state.lock = proposal.lock.clone();
                    }
                    self.send_prevote(&proposal).await;
                }
                Request::RecvQC(qc) => {
                    let qc: AggregatedVote = rlp::decode(&qc.0).expect("decode qc failed");
                    if !qc.is_prevote_qc() && qc.height >= self.state.height {
                        if !qc.block_hash.is_empty() {
                            self.state.height = qc.height + 1;
                            self.state.round = 0;
                            self.state.prev_hash = Hash::from_bytes(qc.block_hash.clone()).unwrap();
                            self.state.proof = Proof {
                                height:     qc.height,
                                round:      qc.round,
                                block_hash: Hash::from_bytes(qc.block_hash.clone()).unwrap(),
                                signature:  qc.signature.signature.clone(),
                                bitmap:     qc.signature.address_bitmap,
                            };
                            self.state.confirm_root = vec![];
                            self.state.receipt_root = vec![];
                            self.state.cycles_used = vec![];
                            self.state.lock = None;
                            self.state.prev_timestamp = time_now();
                        } else if qc.round >= self.state.round {
                            self.state.height = qc.height;
                            self.state.round = qc.round + 1;
                        }
                    }
                }
                Request::RecvVote(vote) => {
                    let vote: SignedVote = rlp::decode(&vote.0).expect("decode vote failed");
                    if vote.vote.height > self.state.height
                        || (vote.vote.height == self.state.height
                            && vote.vote.round > self.state.round)
                    {
                        self.state.height = vote.vote.height;
                        self.state.round = vote.vote.round;
                    }
                }
                Request::RecvChoke(choke) => {
                    let choke: SignedChoke = rlp::decode(&choke.0).expect("decode choke failed");
                    if choke.choke.height > self.state.height
                        || (choke.choke.height == self.state.height
                            && choke.choke.round > self.state.round)
                    {
                        self.state.height = choke.choke.height;
                        self.state.round = choke.choke.round;
                    }
                    self.send_choke(choke.choke.clone(), choke.address.clone())
                        .await;
                }
                Request::RecvHeight(height) => {
                    if *height > self.state.height {
                        self.state.height = *height;
                        self.state.round = 0;
                    }
                }
                _ => panic!("not support yet"),
            }
        }
        self.check_liveness();
    }

    fn check_liveness(&self) {
        let current_time = time_now();
        let gap = current_time - self.state.prev_timestamp;
        if gap > 10 * 60 * 1000 {
            panic!("liveness is seemly broken! do not reach consensus in past 10 min");
        } else if gap > 5 * 60 * 1000 {
            println!("strong warning! do not reach consensus in past 5 min");
        } else if gap > 60 * 1000 {
            println!("warning! do not reach consensus in past 60 s");
        }
    }

    async fn send_prevote(&self, proposal: &Proposal<FixedPill>) {
        let pre_vote = Vote {
            height:     proposal.height,
            round:      proposal.round,
            vote_type:  VoteType::Prevote,
            block_hash: proposal.block_hash.clone(),
        };
        let signed_vote = gen_valid_signed_vote(pre_vote, &self.crypto, &self.pub_key);
        let msg = signed_vote.rlp_bytes();
        let peer_id = PeerId::from_pubkey_bytes(&proposal.proposer)
            .unwrap()
            .into_bytes_ext();
        let _ = self
            .network
            .multicast(
                Context::default(),
                END_GOSSIP_SIGNED_VOTE,
                [peer_id],
                msg,
                Priority::High,
            )
            .await;
    }

    async fn send_choke(&self, choke: Choke, sender: Bytes) {
        let signed_choke = gen_valid_signed_choke(choke, &self.crypto, &self.pub_key);
        let msg = signed_choke.rlp_bytes();
        let peer_id = PeerId::from_pubkey_bytes(&sender).unwrap().into_bytes_ext();
        let _ = self
            .network
            .multicast(
                Context::default(),
                END_GOSSIP_SIGNED_CHOKE,
                [peer_id],
                msg,
                Priority::High,
            )
            .await;
    }
}

async fn send<M, N>(network: &Arc<N>, message: M, ctx: Context, end: &str, behavior: &Behavior)
where
    M: MessageCodec,
    N: Rpc + PeerTrust + Gossip + 'static,
{
    let peer_ids: Vec<_> = behavior
        .send_to
        .iter()
        .map(|pub_key| PeerId::from_pubkey_bytes(pub_key).unwrap().into_bytes_ext())
        .collect();
    let _ = network
        .multicast(ctx.clone(), end, peer_ids, message, behavior.priority)
        .await;
}
