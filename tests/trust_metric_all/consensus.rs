use super::{node::client_node::ClientNodeError, trust_test};

use core_consensus::message::{
    Choke, Proposal, Vote, BROADCAST_HEIGHT, END_GOSSIP_AGGREGATED_VOTE, END_GOSSIP_SIGNED_CHOKE,
    END_GOSSIP_SIGNED_PROPOSAL, END_GOSSIP_SIGNED_VOTE, QC,
};
use protocol::traits::TrustFeedback;

#[test]
fn should_be_disconnected_for_repeated_undecodeable_proposal_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let proposal = Proposal(vec![0000]);
            for _ in 0..4u8 {
                if let Err(ClientNodeError::Unexpected(e)) = client_node
                    .broadcast(END_GOSSIP_SIGNED_PROPOSAL, proposal.clone())
                    .await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Worse(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(feedback) => panic!("unexpected feedback {}", feedback),
                        _ => panic!("fetch trust report"),
                    }
                }

                match client_node.trust_new_interval().await {
                    Ok(_) => (),
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                }
            }

            assert!(!client_node.connected());
        })
    });
}

#[test]
fn should_be_disconnected_for_repeated_undecodeable_vote_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let vote = Vote(vec![0000]);
            for _ in 0..4u8 {
                if let Err(ClientNodeError::Unexpected(e)) = client_node
                    .broadcast(END_GOSSIP_SIGNED_VOTE, vote.clone())
                    .await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Worse(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(feedback) => panic!("unexpected feedback {}", feedback),
                        _ => panic!("fetch trust report"),
                    }
                }

                match client_node.trust_new_interval().await {
                    Ok(_) => (),
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                }
            }

            assert!(!client_node.connected());
        })
    });
}

#[test]
fn should_be_disconnected_for_repeated_undecodeable_qc_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let qc = QC(vec![0000]);
            for _ in 0..4u8 {
                if let Err(ClientNodeError::Unexpected(e)) = client_node
                    .broadcast(END_GOSSIP_AGGREGATED_VOTE, qc.clone())
                    .await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Worse(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(feedback) => panic!("unexpected feedback {}", feedback),
                        _ => panic!("fetch trust report"),
                    }
                }

                match client_node.trust_new_interval().await {
                    Ok(_) => (),
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                }
            }

            assert!(!client_node.connected());
        })
    });
}

#[test]
fn should_be_disconnected_for_repeated_undecodeable_choke_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let choke = Choke(vec![0000]);
            for _ in 0..4u8 {
                if let Err(ClientNodeError::Unexpected(e)) = client_node
                    .broadcast(END_GOSSIP_SIGNED_CHOKE, choke.clone())
                    .await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Worse(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(feedback) => panic!("unexpected feedback {}", feedback),
                        _ => panic!("fetch trust report"),
                    }
                }

                match client_node.trust_new_interval().await {
                    Ok(_) => (),
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                }
            }

            assert!(!client_node.connected());
        })
    });
}

#[test]
fn should_be_disconnected_for_repeated_malicious_new_height_broadcast_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            for _ in 0..4u8 {
                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.broadcast(BROADCAST_HEIGHT, 99u64).await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Bad(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(feedback) => panic!("unexpected feedback {}", feedback),
                        _ => panic!("fetch trust report"),
                    }
                }

                match client_node.trust_new_interval().await {
                    Ok(_) => (),
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                }
            }

            assert!(!client_node.connected());
        })
    });
}
