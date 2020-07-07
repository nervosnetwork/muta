#![allow(clippy::mutable_key_type)]

mod common;
// mod consensus;
mod logger;
mod mempool;
mod node;

use futures::future::BoxFuture;
use node::client_node::{ClientNode, ClientNodeError};
use node::sync::Sync;

use std::panic;

fn trust_test(test: impl FnOnce(ClientNode) -> BoxFuture<'static, ()> + Send + 'static) {
    let (full_port, client_port) = common::available_port_pair();
    let mut rt = tokio::runtime::Runtime::new().expect("create runtime");
    let local = tokio::task::LocalSet::new();

    local.block_on(&mut rt, async move {
        let sync = Sync::new();
        tokio::task::spawn_local(node::full_node::run(full_port, sync.clone()));

        // Wait full node network initialization
        sync.wait().await;

        let handle = tokio::spawn(async move {
            let client_node = node::client_node::connect(full_port, client_port, sync).await;

            test(client_node).await;
        });

        handle.await.expect("test failed");
    });
}

#[test]
fn trust_metric_basic_setup_test() {
    trust_test(move |client_node| {
        Box::pin(async move {
            let block = client_node.get_block(0).await.expect("get genesis");
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

            let report = client_node.trust_new_interval().await.unwrap();
            assert_eq!(report.good_events, 1, "should have 1 good event");
            assert_eq!(report.bad_events, 1, "should have 1 good event");
        })
    });
}

#[test]
fn should_be_disconnected_for_repeated_bad_only_within_four_intervals_from_max_score() {
    trust_test(move |client_node| {
        Box::pin(async move {
            // Repeat at least 30 interval
            let mut count = 30u8;
            while count > 0 {
                count -= 1;

                client_node
                    .trust_twin_event(node::TwinEvent::Good)
                    .await
                    .expect("test trust twin event");

                let report = client_node
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

                match client_node.trust_new_interval().await {
                    Ok(_) => continue,
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
            // Repeat at least 30 interval
            let mut count = 30u8;
            while count > 0 {
                count -= 1;

                client_node
                    .trust_twin_event(node::TwinEvent::Good)
                    .await
                    .expect("test trust twin event");

                let report = client_node
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

                match client_node.trust_new_interval().await {
                    Ok(_) => (),
                    Err(ClientNodeError::NotConnected) => return,
                    Err(e) => panic!("unexpected error {}", e),
                };

                if let Err(ClientNodeError::Unexpected(e)) =
                    client_node.trust_twin_event(node::TwinEvent::Good).await
                {
                    panic!("unexpected {}", e);
                }

                match client_node.trust_new_interval().await {
                    Ok(_) => continue,
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
            let mut base_report = None;

            // Repeat at least 30 interval
            let mut count = 30u8;
            while count > 0 {
                count -= 1;

                client_node
                    .trust_twin_event(node::TwinEvent::Good)
                    .await
                    .expect("test trust twin event");

                let report = client_node
                    .trust_new_interval()
                    .await
                    .expect("test trust new interval");

                if report.score >= 95 {
                    base_report = Some(report);
                    break;
                }
            }

            let mut report = base_report.expect("should have base report");

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
                    Ok(_) => (),
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

            client_node.wait_connected().await;
        })
    });
}
