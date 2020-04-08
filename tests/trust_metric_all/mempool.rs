use super::{
    common,
    node::{self, client_node::ClientNodeRPC},
};

use core_mempool::{MsgNewTxs, END_GOSSIP_NEW_TXS};

#[test]
fn should_be_disconnected_for_invalid_signature_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    std::thread::sleep(std::time::Duration::from_secs(10));

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::make(full_port, client_port).await;
        std::thread::sleep(std::time::Duration::from_secs(10));
        // Add api to fetch current latest block to check whether
        assert!(!client_node.disconnected().await);

        for i in 0..4u8 {
            let stx = common::gen_signed_tx(&client_node.priv_key, 199, false);
            let msg_stxs = MsgNewTxs {
                batch_stxs: vec![stx],
            };

            let ret = client_node.broadcast(END_GOSSIP_NEW_TXS, msg_stxs).await;
            if i == 4 {
                match ret {
                    Ok(_) => panic!("should disconnect"),
                    Err(e) => assert!(e.to_string().contains("unconnected Some(")),
                }
            }
        }
    });
}
