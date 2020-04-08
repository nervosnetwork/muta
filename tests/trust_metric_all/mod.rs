mod common;
mod consensus;
mod logger;
mod mempool;
mod node;

use node::client_node::ClientNodeRPC;

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
