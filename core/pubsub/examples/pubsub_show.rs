use std::thread::spawn;

use futures::executor::block_on;
use futures::future::ready;
use futures::prelude::StreamExt;

use core_pubsub::PubSub;

#[derive(Clone, Debug)]
struct Message {
    header: String,
    body: String,
}

pub fn main() -> Result<(), ()> {
    let mut pubsub = PubSub::builder().build().start();

    let mut sub = pubsub.subscribe::<Message>("test".to_owned())?;

    let sub_two = pubsub.subscribe::<Message>("test".to_owned())?;
    pubsub.unsubscribe("test".to_owned(), sub_two.uuid())?;

    let mut register = pubsub.register();

    let mut pubb = register.publish::<Message>("test".to_owned())?;
    let _test_pubb = spawn(move || {
        let mut count = 1;
        let msg = Message {
            header: "dummy".to_owned(),
            body: "hello world".to_owned(),
        };

        for _ in 0..15 {
            let mut msg = msg.clone();
            msg.header = format!("{}", count);
            let _ = pubb.try_send(msg);

            count += 1;
        }
    });

    block_on(sub.take(5).for_each(|e| {
        println!("{:?}", e);
        ready(())
    }));

    if let Err(err) = pubsub.shutdown() {
        eprintln!("shutdown failure: {:?}", err);
    }

    Ok(())
}
