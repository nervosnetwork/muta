use super::{common, node::client_node::ClientNodeError, trust_test};

use core_mempool::{MsgNewTxs, END_GOSSIP_NEW_TXS};
use protocol::{types::Hash, Bytes};

#[test]
fn should_report_good_on_valid_transaction() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let mut latest_report = client_node.trust_report().await.expect("get report");
            assert_eq!(latest_report.good_events, 0, "should not have any events");
            assert_eq!(latest_report.bad_events, 0, "should not have any events");

            let stx = common::stx_builder().build(&client_node.priv_key);
            let msg_stxs = MsgNewTxs {
                batch_stxs: vec![stx.clone()],
            };

            client_node
                .broadcast(END_GOSSIP_NEW_TXS, msg_stxs)
                .await
                .expect("broadcast stx");

            latest_report = match client_node.until_trust_report_changed(&latest_report).await {
                Ok(report) => report,
                _ => panic!("fetch trust report"),
            };

            assert_eq!(latest_report.good_events, 1, "should have good report");
        })
    });
}

#[test]
fn should_be_disconnected_for_repeated_wrong_signature_only_within_four_intervals() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let mut latest_report = client_node.trust_report().await.expect("get report");

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

                latest_report = match client_node.until_trust_report_changed(&latest_report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                assert_eq!(
                    latest_report.bad_events,
                    1 * latest_report.worse_scalar_ratio,
                    "wrong signature should give worse feedback"
                );

                latest_report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
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
            let mut latest_report = client_node.trust_report().await.expect("get report");

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

                latest_report = match client_node.until_trust_report_changed(&latest_report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                assert_eq!(
                    latest_report.bad_events,
                    1 * latest_report.worse_scalar_ratio,
                    "wrong tx hash should give worse feedback"
                );

                latest_report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
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
            let mut latest_report = client_node.trust_report().await.expect("get report");

            let stx = common::stx_builder()
                .payload("trust-metric".repeat(1000000))
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

                latest_report = match client_node.until_trust_report_changed(&latest_report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                assert_eq!(
                    latest_report.bad_events, 1,
                    "exceed tx size limit should give bad feedback"
                );

                latest_report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
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
            let mut latest_report = client_node.trust_report().await.expect("get report");

            let stx = common::stx_builder()
                .cycles_limit(999999999999)
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

                latest_report = match client_node.until_trust_report_changed(&latest_report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                assert_eq!(
                    latest_report.bad_events, 1,
                    "exceed cycles limit should give bad feedback"
                );

                latest_report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
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
            let mut latest_report = client_node.trust_report().await.expect("get report");

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

                latest_report = match client_node.until_trust_report_changed(&latest_report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                assert_eq!(
                    latest_report.bad_events,
                    1 * latest_report.worse_scalar_ratio,
                    "wrong chain id should give worse feedback"
                );

                latest_report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
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
            let mut latest_report = client_node.trust_report().await.expect("get report");

            let stx = common::stx_builder()
                .timeout(9999999)
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

                latest_report = match client_node.until_trust_report_changed(&latest_report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                assert_eq!(
                    latest_report.bad_events, 1,
                    "larger timeout should give bad feedback"
                );

                latest_report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
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
            let mut latest_report = client_node.trust_report().await.expect("get report");

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

                latest_report = match client_node.until_trust_report_changed(&latest_report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                assert_eq!(
                    latest_report.bad_events, 1,
                    "smaller timeout should give bad feedback"
                );

                latest_report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                }
            }

            assert!(!client_node.connected());
        })
    });
}
