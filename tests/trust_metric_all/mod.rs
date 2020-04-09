mod common;
mod consensus;
mod logger;
mod mempool;
mod node;

#[test]
fn trust_metric_basic_setup_test() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;

        let block = client_node.genesis_block().await.expect("get genesis");
        assert_eq!(block.header.height, 0);
    });
}

#[test]
fn should_have_working_trust_diagnostic() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
        client_node
            .trust_twin_event()
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
    });
}
