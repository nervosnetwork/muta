mod common;

use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
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
const BROADCAST_TEST_TIMEOUT: u64 = 30;

enum TestResult {
    TimeOut,
    Success,
}

struct NewsReader {
    sent:    AtomicBool,
    done_tx: UnboundedSender<()>,
}

impl NewsReader {
    pub fn new(done_tx: UnboundedSender<()>) -> Self {
        NewsReader {
            sent: AtomicBool::new(false),
            done_tx,
        }
    }

    pub fn sent(&self) -> bool {
        self.sent.load(Ordering::SeqCst)
    }

    pub fn set_sent(&self) {
        self.sent.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl MessageHandler for NewsReader {
    type Message = String;

    async fn process(&self, _ctx: Context, msg: Self::Message) -> ProtocolResult<()> {
        if !self.sent() {
            assert_eq!(&msg, TEST_MESSAGE);
            self.done_tx.unbounded_send(()).expect("news reader done");
            self.set_sent();
        }

        ProtocolResult::Ok(())
    }
}

// FIXME: sometimes timeout
#[runtime::test(runtime_tokio::Tokio)]
#[ignore]
async fn broadcast() {
    env_logger::init();

    let (test_tx, mut test_rx) = unbounded();

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
    thread::sleep(Duration::from_secs(3));

    // Loop broadcast test message until all peers receive test message
    let test_tx_clone = test_tx.clone();
    runtime::spawn(async move {
        let ctx = Context::new();
        let end = END_TEST_BROADCAST;
        let msg = TEST_MESSAGE.to_owned();
        let start = SystemTime::now();

        loop {
            if SystemTime::now()
                .duration_since(start)
                .expect("duration")
                .as_secs()
                > BROADCAST_TEST_TIMEOUT
            {
                test_tx_clone
                    .unbounded_send(TestResult::TimeOut)
                    .expect("timeout send");
            }

            broadcaster
                .broadcast(ctx.clone(), end, msg.clone(), Priority::Normal)
                .await
                .expect("gossip broadcast");

            thread::sleep(Duration::from_secs(2));
        }
    });

    runtime::spawn(async move {
        bootstrap_done.next().await.expect("bootstrap done");
        alpha_done.next().await.expect("alpha done");
        brova_done.next().await.expect("brova done");

        test_tx
            .unbounded_send(TestResult::Success)
            .expect("success send");
    });

    match test_rx.next().await {
        Some(TestResult::TimeOut) => panic!("timeout"),
        Some(TestResult::Success) => (),
        None => panic!("fail"),
    }
}
