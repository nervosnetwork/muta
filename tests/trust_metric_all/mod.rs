mod common;
mod consensus;
mod mempool;
mod node;

use node::client_node::ClientNodeRPC;

#[test]
fn trust_metric_basic_setup_test() {
    let _handle = std::thread::spawn(move || {
        node::full_node::run(1337);
    });

    std::thread::sleep(std::time::Duration::from_secs(5));

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::make(1337u16, 9527u16).await;

        std::thread::sleep(std::time::Duration::from_secs(5));

        let block = client_node.genesis_block().await.expect("get genesis");
        assert_eq!(block.header.height, 0);
    });
}
