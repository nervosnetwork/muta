mod common;
mod consensus;
mod full_node;
mod mempool;
mod test_node;

use test_node::TestNodeRPC;

const FULL_NODE_PUBKEY: &str = "031288a6788678c25952eba8693b2f278f66e2187004b64ac09416d07f83f96d5b";
const FULL_NODE_CHAIN_ADDR: &str = "0xf8389d774afdad8755ef8e629e5a154fddc6325a";
const FULL_NODE_ADDR: &str = "127.0.0.1:1337";

#[test]
fn trust_metric_basic_setup_test() {
    let _handle = std::thread::spawn(move || {
        full_node::run();
    });

    std::thread::sleep(std::time::Duration::from_secs(10));

    let full_node = test_node::FullNode {
        pubkey:     FULL_NODE_PUBKEY.to_owned(),
        chain_addr: FULL_NODE_CHAIN_ADDR.to_owned(),
        addr:       FULL_NODE_ADDR.to_owned(),
    };

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let test_node = test_node::make(full_node, 9527u16).await;

        std::thread::sleep(std::time::Duration::from_secs(10));

        let block = test_node.genesis_block().await.expect("get genesis");
        assert_eq!(block.header.height, 0);
    });
}
