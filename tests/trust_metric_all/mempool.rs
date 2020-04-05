use super::{common, full_node, test_node};
use test_node::TestNodeRPC;

use core_mempool::{MsgNewTxs, END_GOSSIP_NEW_TXS};
use protocol::traits::{Context, Priority, Gossip};

const FULL_NODE_PUBKEY: &str = "031288a6788678c25952eba8693b2f278f66e2187004b64ac09416d07f83f96d5b";
const FULL_NODE_CHAIN_ADDR: &str = "0xf8389d774afdad8755ef8e629e5a154fddc6325a";
const FULL_NODE_ADDR: &str = "127.0.0.1:1337";

#[test]
fn should_be_disconnected_for_invalid_signature_within_four_intervals() {
    let _handle = std::thread::spawn(move || {
        full_node::run();
    });

    std::thread::sleep(std::time::Duration::from_secs(10));

    let full_node = test_node::FullNode {
        pubkey: FULL_NODE_PUBKEY.to_owned(),
        chain_addr: FULL_NODE_CHAIN_ADDR.to_owned(),
        addr:   FULL_NODE_ADDR.to_owned(),
    };

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let test_node = test_node::make(full_node, 9527u16).await;

        std::thread::sleep(std::time::Duration::from_secs(10));

        assert!(!test_node.disconnected().await);
    });
}
