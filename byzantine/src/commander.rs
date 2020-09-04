use bytes::Bytes;
use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    stream::StreamExt,
};
use tokio::time::{self, Duration};

use core_consensus::message::{
    BROADCAST_HEIGHT, END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_CHOKE,
    END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE,
};
use protocol::traits::{Context, Priority};

use crate::behaviors::{Behavior, MessageType, PullTxs, Request};
use crate::config::Generators;
use crate::strategy::{BehaviorGenerator, DefaultStrategy, Strategy};

pub struct Commander {
    generators:   Generators,
    pub_key_list: Vec<Bytes>,
    to_worker:    UnboundedSender<(Context, Vec<Behavior>)>,
    from_network: UnboundedReceiver<(Context, Request)>,
}

impl Commander {
    pub fn new(
        generators: Generators,
        pub_key_list: Vec<Bytes>,
        to_worker: UnboundedSender<(Context, Vec<Behavior>)>,
        from_network: UnboundedReceiver<(Context, Request)>,
    ) -> Self {
        Commander {
            generators,
            pub_key_list,
            to_worker,
            from_network,
        }
    }

    pub async fn run(mut self) {
        let mut list = self.generators.list.clone();
        add_primitive_generator(&mut list);
        let strategy = DefaultStrategy::new(self.pub_key_list.clone(), list);
        let interval = self.generators.interval;

        let mut cnt = 0;
        loop {
            let mut delay = time::delay_for(Duration::from_millis(interval));
            tokio::select! {
                _ = &mut delay => {
                    let behaviors = strategy.get_behaviors(None);
                    cnt += behaviors.len();
                    println!("commander is working, accumulative gen {} behaviors", cnt);
                    let _ = self.to_worker.unbounded_send((Context::default(), behaviors));
                }

                Some((ctx, request)) = self.from_network.next() => {
                    let behaviors = strategy.get_behaviors(Some(request));
                    cnt += behaviors.len();
                    println!("commander receive message from network, accumulative gen {} behaviors", cnt);
                    let _ = self.to_worker.unbounded_send((ctx, behaviors));
                }
            }
        }
    }
}

fn add_primitive_generator(list: &mut Vec<BehaviorGenerator>) {
    let valid_recv_proposal_generator = BehaviorGenerator {
        req_end:     Some(END_GOSSIP_SIGNED_PROPOSAL.to_string()),
        msg_type:    MessageType::RecvProposal(PullTxs::Valid),
        probability: 1.0,
        num_range:   (1, 2),
        priority:    Priority::High,
    };
    list.push(valid_recv_proposal_generator);

    let valid_recv_vote_generator = BehaviorGenerator {
        req_end:     Some(END_GOSSIP_SIGNED_VOTE.to_string()),
        msg_type:    MessageType::RecvVote,
        probability: 1.0,
        num_range:   (1, 2),
        priority:    Priority::High,
    };
    list.push(valid_recv_vote_generator);

    let valid_recv_qc_generator = BehaviorGenerator {
        req_end:     Some(END_GOSSIP_AGGREGATED_VOTE.to_string()),
        msg_type:    MessageType::RecvQC,
        probability: 1.0,
        num_range:   (1, 2),
        priority:    Priority::High,
    };
    list.push(valid_recv_qc_generator);

    let valid_recv_choke_generator = BehaviorGenerator {
        req_end:     Some(END_GOSSIP_SIGNED_CHOKE.to_string()),
        msg_type:    MessageType::RecvChoke,
        probability: 1.0,
        num_range:   (1, 2),
        priority:    Priority::High,
    };
    list.push(valid_recv_choke_generator);

    let valid_recv_height_generator = BehaviorGenerator {
        req_end:     Some(BROADCAST_HEIGHT.to_string()),
        msg_type:    MessageType::RecvHeight,
        probability: 1.0,
        num_range:   (1, 2),
        priority:    Priority::High,
    };
    list.push(valid_recv_height_generator);
}
