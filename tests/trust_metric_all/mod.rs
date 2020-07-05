#![allow(clippy::mutable_key_type)]

mod common;
mod consensus;
mod logger;
mod mempool;
mod node;

use futures::future::BoxFuture;
use node::client_node::{ClientNode, ClientNodeError};

use std::panic;

fn trust_test(test: impl FnOnce(ClientNode) -> BoxFuture<'static, ()> + Send + 'static) {
    let (full_port, client_port) = common::available_port_pair();
    let mut rt = tokio::runtime::Runtime::new().expect("create runtime");
    let local = tokio::task::LocalSet::new();

    local.block_on(&mut rt, async move {
        let full_node_running = common::RunningStatus::new();
        tokio::task::spawn_local(node::full_node::run(full_port, full_node_running.clone()));

        // Wait for full node network initialization
        full_node_running.wait().await;

        let handle = tokio::spawn(async move {
            let client_node =
                node::client_node::connect(full_port, client_port, full_node_running).await;

            test(client_node).await;
        });

        handle.await.expect("test pass");
    });
}

#[test]
fn trust_metric_basic_setup_test() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let block = client_node.genesis_block().await.expect("get genesis");
            assert_eq!(block.header.height, 0);
        })
    });
}

#[test]
fn should_have_working_trust_diagnostic() {
    trust_test(move |client_node| {
        Box::pin(async move {
            client_node
                .trust_twin_event(node::TwinEvent::Both)
                .await
                .expect("test trust twin event");

            let report = client_node
                .trust_report()
                .await
                .expect("fetch trust report");
            assert_eq!(report.good_events, 1, "should have 1 good event");
            assert_eq!(report.bad_events, 1, "should have 1 bad event");

            client_node
                .trust_new_interval()
                .await
                .expect("test trust new interval");
            let report = client_node
                .trust_report()
                .await
                .expect("fetch trust report");
            assert_eq!(report.good_events, 0, "should have 0 good event");
            assert_eq!(report.bad_events, 0, "should have 0 bad event");
        })
    });
}

#[test]
fn should_be_disconnected_for_repeated_bad_only_within_four_intervals_from_max_score() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let mut report = client_node
                .trust_report()
                .await
                .expect("fetch trust report");

            // Repeat at least 30 interval
            let mut count = 30u8;
            while count > 0 {
                count -= 1;

                client_node
                    .trust_twin_event(node::TwinEvent::Good)
                    .await
                    .expect("test trust twin event");

                report = client_node
                    .trust_new_interval()
                    .await
                    .expect("test trust new interval");

                if report.score >= 95 {
                    break;
                }
            }

            for _ in 0..4u8 {
                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.trust_twin_event(node::TwinEvent::Bad).await
                {
                    panic!("unexpected {}", e);
                }

                match client_node.until_trust_report_changed(&report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                report = match client_node.trust_new_interval().await {
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
fn should_be_disconnected_for_repeated_s_strategy_within_17_intervals_from_max_score() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let mut report = client_node
                .trust_report()
                .await
                .expect("fetch trust report");

            // Repeat at least 30 interval
            let mut count = 30u8;
            while count > 0 {
                count -= 1;

                client_node
                    .trust_twin_event(node::TwinEvent::Good)
                    .await
                    .expect("test trust twin event");

                report = client_node
                    .trust_new_interval()
                    .await
                    .expect("test trust new interval");

                if report.score >= 95 {
                    break;
                }
            }

            for _ in 0..17u8 {
                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.trust_twin_event(node::TwinEvent::Worse).await
                {
                    panic!("unexpected {}", e);
                }

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.trust_twin_event(node::TwinEvent::Good).await
                {
                    panic!("unexpected {}", e);
                }

                match client_node.until_trust_report_changed(&report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                };

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.trust_twin_event(node::TwinEvent::Good).await
                {
                    panic!("unexpected {}", e);
                }

                match client_node.until_trust_report_changed(&report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                };
            }

            assert!(!client_node.connected());
        })
    });
}

#[test]
fn should_keep_connected_for_z_strategy_but_have_lower_score() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let mut report = client_node
                .trust_report()
                .await
                .expect("fetch trust report");

            // Repeat at least 30 interval
            let mut count = 30u8;
            while count > 0 {
                count -= 1;

                client_node
                    .trust_twin_event(node::TwinEvent::Good)
                    .await
                    .expect("test trust twin event");

                report = client_node
                    .trust_new_interval()
                    .await
                    .expect("test trust new interval");

                if report.score >= 95 {
                    break;
                }
            }

            for _ in 0..100u8 {
                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.trust_twin_event(node::TwinEvent::Bad).await
                {
                    panic!("unexpected {}", e);
                }

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.trust_twin_event(node::TwinEvent::Good).await
                {
                    panic!("unexpected {}", e);
                }

                match client_node.until_trust_report_changed(&report).await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected {}", e),
                };

                let latest_report = match client_node.trust_new_interval().await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                };

                assert!(latest_report.score <= report.score);
                report = latest_report;
            }

            assert!(client_node.connected(), "should be connected");
        })
    });
}

#[test]
fn should_able_to_reconnect_after_trust_metric_soft_ban() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let mut count = 30u8;

            while count > 0 {
                count -= 1;

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.trust_twin_event(node::TwinEvent::Bad).await
                {
                    panic!("unexpected {}", e);
                }

                match client_node.trust_new_interval().await {
                    Ok(report) => report,
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                };

                if !client_node.connected() {
                    break;
                }
            }

            assert!(!client_node.connected(), "should be disconnected");

            // Ensure we we dont sleep longer than back-off time
            let soft_ban_duration =
                node::consts::NETWORK_SOFT_BAND_DURATION.expect("soft ban") * 2u64;
            tokio::time::delay_for(std::time::Duration::from_secs(soft_ban_duration)).await;

            count = 30u8;
            while count > 0 {
                count -= 1;

                if client_node.connected() {
                    return;
                }
            }

            panic!("should be reconnected");
        })
    });
}
