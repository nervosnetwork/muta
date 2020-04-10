use super::{
    common,
    node::{self, client_node::ClientNodeError},
};

use core_mempool::{MsgNewTxs, END_GOSSIP_NEW_TXS};

#[test]
fn should_be_disconnected_for_invalid_signature_within_four_intervals() {
    let (full_port, client_port) = common::available_port_pair();
    let _handle = std::thread::spawn(move || {
        node::full_node::run(full_port);
    });

    let mut runtime = tokio::runtime::Runtime::new().expect("create runtime");
    runtime.block_on(async move {
        let client_node = node::client_node::connect(full_port, client_port).await;
        let mut last_report = client_node.trust_report().await.expect("get report");

        for _ in 0..4u8 {
            let stx = common::gen_signed_tx(&client_node.priv_key, 199, false);
            let msg_stxs = MsgNewTxs {
                batch_stxs: vec![stx],
            };

            if let Err(ClientNodeError::Unexpected(e)) =
                client_node.broadcast(END_GOSSIP_NEW_TXS, msg_stxs).await
            {
                panic!("unexpected {}", e);
            }

            last_report = match client_node.until_trust_report_changed(&last_report).await {
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
