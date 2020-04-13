mod common;
mod consensus;
mod logger;
mod mempool;
mod node;

use futures::future::BoxFuture;
use node::client_node::ClientNode;

const FULL_NODE_SETUP_WAIT_TIME: u64 = 5;

fn trust_test(test: impl FnOnce(ClientNode) -> BoxFuture<'static, ()> + Send + 'static) {
    let (full_port, client_port) = common::available_port_pair();
    let mut rt = tokio::runtime::Runtime::new().expect("create runtime");
    let local = tokio::task::LocalSet::new();

    local.block_on(&mut rt, async move {
        tokio::task::spawn_local(node::full_node::run(full_port));
        // Sleep a while for full node network to running, otherwise will
        // trigger network retry back off.
        tokio::time::delay_for(std::time::Duration::from_secs(FULL_NODE_SETUP_WAIT_TIME)).await;

        let handle = tokio::spawn(async move {
            let client_node = node::client_node::connect(full_port, client_port).await;

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
        })
    });
}
