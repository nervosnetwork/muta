mod common;

use std::{thread, time};

use async_trait::async_trait;
use derive_more::Constructor;
use futures::{
    channel::mpsc::{unbounded, UnboundedSender},
    stream::StreamExt,
};

use protocol::{
    traits::{Context, Gossip, MessageHandler, Priority},
    ProtocolResult,
};

const END_TEST_BROADCAST: &str = "/gossip/test/message";
const TEST_MESSAGE: &str = "spike lee action started";

#[derive(Constructor)]
struct NewsReader {
    done_tx: UnboundedSender<()>,
}

#[async_trait]
impl MessageHandler for NewsReader {
    type Message = String;

    async fn process(&self, _ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        assert_eq!(&msg, TEST_MESSAGE);
        self.done_tx.unbounded_send(()).expect("news reader done");

        ProtocolResult::Ok(())
    }
}

#[runtime::test(runtime_tokio::Tokio)]
async fn test_broadcast() {
    env_logger::init();

    // Init bootstrap node
    let mut bootstrap = common::setup_bootstrap();
    let (done_tx, mut bootstrap_done) = unbounded();

    bootstrap
        .register_endpoint_handler(END_TEST_BROADCAST, Box::new(NewsReader::new(done_tx)))
        .expect("bootstrap register news reader");

    runtime::spawn(bootstrap);

    // Init peer alpha
    let mut alpha = common::setup_peer(common::BOOTSTRAP_PORT + 1);
    let (done_tx, mut alpha_done) = unbounded();

    alpha
        .register_endpoint_handler(END_TEST_BROADCAST, Box::new(NewsReader::new(done_tx)))
        .expect("alpha register news reader");

    runtime::spawn(alpha);

    // Init peer brova
    let mut brova = common::setup_peer(common::BOOTSTRAP_PORT + 2);
    let (done_tx, mut brova_done) = unbounded();

    brova
        .register_endpoint_handler(END_TEST_BROADCAST, Box::new(NewsReader::new(done_tx)))
        .expect("brova register news reader");

    runtime::spawn(brova);

    // Init peer charlie
    let charlie = common::setup_peer(common::BOOTSTRAP_PORT + 3);
    let broadcaster = charlie.handle();

    runtime::spawn(charlie);

    // Sleep a while for bootstrap phrase, so peers can connect to each other
    thread::sleep(time::Duration::from_secs(3));

    // Loop broadcast test message until all peers receive test message
    runtime::spawn(async move {
        let ctx = Context::new();
        let end = END_TEST_BROADCAST;
        let msg = TEST_MESSAGE.to_owned();

        loop {
            broadcaster
                .broadcast(ctx.clone(), end, msg.clone(), Priority::High)
                .await
                .expect("gossip broadcast");
            thread::sleep(time::Duration::from_secs(1));
        }
    });

    bootstrap_done.next().await.expect("bootstrap done");
    alpha_done.next().await.expect("alpha done");
    brova_done.next().await.expect("brova done");
}
