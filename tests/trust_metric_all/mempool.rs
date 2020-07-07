use super::{common, node::client_node::ClientNodeError, trust_test};

use core_mempool::{MsgNewTxs, END_GOSSIP_NEW_TXS};
use protocol::{traits::TrustFeedback, types::Hash, Bytes};

#[test]
fn should_report_good_on_valid_transaction() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let stx = common::stx_builder().build(&client_node.priv_key);
            let msg_stxs = MsgNewTxs {
                batch_stxs: vec![stx.clone()],
            };

            client_node
                .broadcast(END_GOSSIP_NEW_TXS, msg_stxs)
                .await
                .expect("broadcast stx");

            match client_node.until_trust_processed().await {
                Ok(TrustFeedback::Good) => return,
                Ok(_) => panic!("should be good report"),
                _ => panic!("fetch trust report"),
            }
        })
    });
}

#[test]
fn should_be_disconnected_for_repeated_wrong_signature_only_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let mut stx = common::stx_builder().build(&client_node.priv_key);
            stx.signature = Bytes::from(vec![0]);
            for _ in 0..4u8 {
                let msg_stxs = MsgNewTxs {
                    batch_stxs: vec![stx.clone()],
                };

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.broadcast(END_GOSSIP_NEW_TXS, msg_stxs).await
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
fn should_be_disconnected_for_repeated_wrong_tx_hash_only_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let mut stx = common::stx_builder().build(&client_node.priv_key);
            stx.tx_hash = Hash::digest(Bytes::from(vec![0]));
            for _ in 0..4u8 {
                let msg_stxs = MsgNewTxs {
                    batch_stxs: vec![stx.clone()],
                };

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.broadcast(END_GOSSIP_NEW_TXS, msg_stxs).await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Worse(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(_) => panic!("should be good report"),
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
fn should_be_disconnected_for_repeated_exceed_tx_size_limit_only_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let stx = common::stx_builder()
                .payload("trust-metric".repeat(1_000))
                .build(&client_node.priv_key);
            for _ in 0..4u8 {
                let msg_stxs = MsgNewTxs {
                    batch_stxs: vec![stx.clone()],
                };

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.broadcast(END_GOSSIP_NEW_TXS, msg_stxs).await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Bad(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(_) => panic!("should be good report"),
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
fn should_be_disconnected_for_repeated_exceed_cycles_limit_only_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let stx = common::stx_builder()
                .cycles_limit(999_999_999_999)
                .build(&client_node.priv_key);
            for _ in 0..4u8 {
                let msg_stxs = MsgNewTxs {
                    batch_stxs: vec![stx.clone()],
                };

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.broadcast(END_GOSSIP_NEW_TXS, msg_stxs).await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Bad(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(_) => panic!("should be good report"),
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
fn should_be_disconnected_for_repeated_wrong_chain_id_only_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let stx = common::stx_builder()
                .chain_id(Bytes::from(vec![0]))
                .build(&client_node.priv_key);
            for _ in 0..4u8 {
                let msg_stxs = MsgNewTxs {
                    batch_stxs: vec![stx.clone()],
                };

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.broadcast(END_GOSSIP_NEW_TXS, msg_stxs).await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Worse(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(_) => panic!("should be good report"),
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
fn should_be_disconnected_for_repeated_timeout_larger_than_gap_only_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let stx = common::stx_builder()
                .timeout(9_999_999)
                .build(&client_node.priv_key);
            for _ in 0..4u8 {
                let msg_stxs = MsgNewTxs {
                    batch_stxs: vec![stx.clone()],
                };

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.broadcast(END_GOSSIP_NEW_TXS, msg_stxs).await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Bad(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(_) => panic!("should be good report"),
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
fn should_be_disconnected_for_repeated_timeout_smaller_than_latest_height_only_within_four_intervals(
) {
    trust_test(move |client_node| {
        Box::pin(async move {
            let stx = common::stx_builder()
                .timeout(0)
                .build(&client_node.priv_key);
            for _ in 0..4u8 {
                let msg_stxs = MsgNewTxs {
                    batch_stxs: vec![stx.clone()],
                };

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.broadcast(END_GOSSIP_NEW_TXS, msg_stxs).await
                {
                    panic!("unexpected {}", e);
                }

                loop {
                    match client_node.until_trust_processed().await {
                        Ok(TrustFeedback::Bad(_)) => break,
                        Ok(TrustFeedback::Neutral) => continue,
                        Ok(_) => panic!("should be good report"),
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
