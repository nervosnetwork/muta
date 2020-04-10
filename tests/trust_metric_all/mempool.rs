use super::{
    common,
    node::{self, client_node::ClientNodeError, consts},
};

use core_mempool::{MsgNewTxs, END_GOSSIP_NEW_TXS};
use protocol::{types::Hash, Bytes};

#[test]
fn should_report_good_on_valid_transaction() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
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
    });
}

#[test]
fn should_be_disconnected_for_repeated_wrong_signature_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
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

            match client_node.trust_new_interval().await {
                Ok(()) => (),
                Err(ClientNodeError::NotConnected) => return,
                Err(e) => panic!("unexpected error {}", e),
            }
        }
    });
}

#[test]
fn should_be_disconnected_for_repeated_wrong_tx_hash_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
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

            match client_node.trust_new_interval().await {
                Ok(()) => (),
                Err(ClientNodeError::NotConnected) => return,
                Err(e) => panic!("unexpected error {}", e),
            }
        }
    });
}

#[test]
fn should_be_disconnected_for_repeated_exceed_mempool_size_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
        let mut latest_report = client_node.trust_report().await.expect("get report");

        let stxs = (0..consts::MEMPOOL_POOL_SIZE + 1)
            .map(|_| common::stx_builder().build(&client_node.priv_key))
            .collect::<Vec<_>>();
        for _ in 0..4u8 {
            let msg_stxs = MsgNewTxs {
                batch_stxs: stxs.clone(),
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

            match client_node.trust_new_interval().await {
                Ok(()) => (),
                Err(ClientNodeError::NotConnected) => return,
                Err(e) => panic!("unexpected error {}", e),
            }
        }
    });
}

#[test]
fn should_be_disconnected_for_repeated_exceed_tx_size_limit_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
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

            match client_node.trust_new_interval().await {
                Ok(()) => (),
                Err(ClientNodeError::NotConnected) => return,
                Err(e) => panic!("unexpected error {}", e),
            }
        }
    });
}

#[test]
fn should_be_disconnected_for_repeated_exceed_cycles_limit_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
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

            match client_node.trust_new_interval().await {
                Ok(()) => (),
                Err(ClientNodeError::NotConnected) => return,
                Err(e) => panic!("unexpected error {}", e),
            }
        }
    });
}

#[test]
fn should_be_disconnected_for_repeated_wrong_chain_id_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
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

            match client_node.trust_new_interval().await {
                Ok(()) => (),
                Err(ClientNodeError::NotConnected) => return,
                Err(e) => panic!("unexpected error {}", e),
            }
        }
    });
}

#[test]
fn should_be_disconnected_for_repeated_timeout_larger_than_gap_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
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

            match client_node.trust_new_interval().await {
                Ok(()) => (),
                Err(ClientNodeError::NotConnected) => return,
                Err(e) => panic!("unexpected error {}", e),
            }
        }
    });
}

#[test]
fn should_be_disconnected_for_repeated_timeout_smaller_than_latest_height_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
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

            match client_node.trust_new_interval().await {
                Ok(()) => (),
                Err(ClientNodeError::NotConnected) => return,
                Err(e) => panic!("unexpected error {}", e),
            }
        }
    });
}
