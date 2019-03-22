pub mod call;

use futures::sync::mpsc::{channel, Receiver, Sender};
use futures::Stream;

use std::collections::HashMap;
use std::sync::Arc;
use std::thread::{spawn, JoinHandle};

use core_types::{Block, SignedTransaction};

use crate::call::Request;

// TODO stop signal
// const SIGNAL_CHANNEL_SIZE: usize = 1;
const REGISTER_CHANNEL_SIZE: usize = 2;
const NOTIFY_CHANNEL_SIZE: usize = 64;

type TransactionRegisterRequest = Request<String, Receiver<Arc<SignedTransaction>>>;
type BlockRegisterRequest = Request<String, Receiver<Arc<Block>>>;
type TransactionNotifyRequest = Request<Arc<SignedTransaction>, ()>;
type BlockNotifyRequest = Request<Arc<Block>, ()>;

enum NotifyRequest {
    TRR(TransactionRegisterRequest),
    BRR(BlockRegisterRequest),
    TNR(TransactionNotifyRequest),
    BNR(BlockNotifyRequest),
}

pub struct NotifyController {
    transaction_register: Sender<TransactionRegisterRequest>,
    block_register: Sender<BlockRegisterRequest>,

    transaction_sender: Sender<TransactionNotifyRequest>,
    block_sender: Sender<BlockNotifyRequest>,
}

pub struct NotifyService {
    transaction_register_receiver: Receiver<TransactionRegisterRequest>,
    block_register_receiver: Receiver<BlockRegisterRequest>,

    transaction_receiver: Receiver<TransactionNotifyRequest>,
    block_receiver: Receiver<BlockNotifyRequest>,
}

impl NotifyController {
    // Registers a new transaction-subscriber to receive new transactions
    pub fn subscribe_transaction<S: ToString>(
        &mut self,
        name: S,
    ) -> Receiver<Arc<SignedTransaction>> {
        Request::call(&mut self.transaction_register, name.to_string())
            .expect("Failed to subscribe new transactions")
    }

    // Registers a new block-subscriber to receive new blocks
    pub fn subscribe_block<S: ToString>(&mut self, name: S) -> Receiver<Arc<Block>> {
        Request::call(&mut self.block_register, name.to_string())
            .expect("Failed to subscribe new blocks")
    }

    // Send new transaction to NotifyService, which will publish it to all
    // transaction-subscribers
    pub fn notify_transaction(&mut self, tx: Arc<SignedTransaction>) {
        Request::call(&mut self.transaction_sender, tx).expect("Failed to notify new transactions")
    }

    // Send new transaction to NotifyService, which will publish it to all
    // block-subscribers
    pub fn notify_block(&mut self, block: Arc<Block>) {
        Request::call(&mut self.block_sender, block).expect("Failed to notify new blocks")
    }
}

impl NotifyService {
    // Creates a pair (NotifyService, NotifyController), NotifyService will
    // serve on background.
    pub fn start() -> (JoinHandle<()>, NotifyController) {
        let (transaction_register, transaction_register_receiver) = channel(REGISTER_CHANNEL_SIZE);
        let (transaction_sender, transaction_receiver) = channel(NOTIFY_CHANNEL_SIZE);
        let (block_register, block_register_receiver) = channel(REGISTER_CHANNEL_SIZE);
        let (block_sender, block_receiver) = channel(NOTIFY_CHANNEL_SIZE);

        let notify_controller = NotifyController {
            transaction_register,
            block_register,
            transaction_sender,
            block_sender,
        };
        let notify_service = NotifyService {
            transaction_register_receiver,
            block_register_receiver,
            transaction_receiver,
            block_receiver,
        };
        let handle = spawn(move || {
            notify_service.serve();
        });

        (handle, notify_controller)
    }

    fn serve(self) {
        // Create subscriber: #{name => sender}
        let mut transaction_subscribers = HashMap::new();
        let mut block_subscribers = HashMap::new();

        // Cast futures::receivers' item-types into `NotifyRequest`, so that
        // be able to use `select` to listen several channels at the same time
        let trr = self.transaction_register_receiver.map(NotifyRequest::TRR);
        let brr = self.block_register_receiver.map(NotifyRequest::BRR);
        let tnr = self.transaction_receiver.map(NotifyRequest::TNR);
        let bnr = self.block_receiver.map(NotifyRequest::BNR);

        // Loop to listen and handle requests
        let selector = trr
            .select(brr)
            .select(tnr)
            .select(bnr)
            .for_each(move |req| {
                match req {
                    NotifyRequest::TRR(Request {
                        arguments: name,
                        responder,
                    }) => {
                        let (sender, receiver) =
                            channel::<Arc<SignedTransaction>>(NOTIFY_CHANNEL_SIZE);
                        println!("NotifyService add a new transaction-subscriber: {}", name);
                        transaction_subscribers.insert(name, sender);
                        let _ = responder.send(receiver);
                    }
                    NotifyRequest::BRR(Request {
                        arguments: name,
                        responder,
                    }) => {
                        let (sender, receiver) = channel::<Arc<Block>>(NOTIFY_CHANNEL_SIZE);
                        println!("NotifyService add a new block-subscriber: {}", name);
                        block_subscribers.insert(name, sender);
                        let _ = responder.send(receiver);
                    }
                    NotifyRequest::TNR(Request {
                        arguments: tx,
                        responder,
                    }) => {
                        let _ = responder.send(());
                        for subscriber in transaction_subscribers.values() {
                            let _ = subscriber.clone().try_send(Arc::clone(&tx));
                        }
                    }
                    NotifyRequest::BNR(Request {
                        arguments: block,
                        responder,
                    }) => {
                        let _ = responder.send(());
                        for subscriber in block_subscribers.values() {
                            let _ = subscriber.clone().try_send(Arc::clone(&block));
                        }
                    }
                }
                Ok(())
            });
        tokio::run(selector);
    }
}

#[cfg(test)]
mod tests {
    use futures::{Future, Stream};

    use std::sync::Arc;

    use core_types::SignedTransaction;

    use super::NotifyService;

    #[test]
    fn test_notify_transaction() {
        // Start NotifyService and return NotifyController
        let (_, mut controller) = NotifyService::start();

        let mut transaction1 = SignedTransaction::default();
        let mut transaction2 = SignedTransaction::default();
        transaction1.untx.transaction.quota = 1;
        transaction2.untx.transaction.quota = 2;

        // Register with name "foo" to receive new transactions
        // from `foo_tx_receiver`
        let foo_tx_receiver = controller.subscribe_transaction("foo");

        // Send new transaction via `NotifyController::notify_transaction`.
        // Then subscribers will receive the new transactions
        controller.notify_transaction(Arc::new(transaction1));
        let _ = foo_tx_receiver
            .map(|tx| {
                assert_eq!(tx.untx.transaction.quota, 1);
            })
            .into_future()
            .wait();

        // New subscriber will not receive the staled transactions, but
        // new transactions
        let bar_tx_receiver = controller.subscribe_transaction("bar");
        controller.notify_transaction(Arc::new(transaction2));
        let _ = bar_tx_receiver
            .map(|tx| {
                assert_eq!(tx.untx.transaction.quota, 2);
            })
            .into_future()
            .wait();
    }
}
