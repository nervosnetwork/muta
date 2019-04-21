use crate::p2p::{Builder, Config, Message, PackedMessage, ServiceWorker, Task};

use core_p2p::{transmission::CastMessage, DefaultPeerManager};

use futures::prelude::Stream;
use futures::sync::mpsc::{Receiver, Sender};
use log::{debug, error};
use tentacle::builder::ServiceBuilder;
use tentacle::context::ServiceContext;
use tentacle::service::{DialProtocol, ServiceError, ServiceEvent};
use tentacle::traits::ServiceHandle;

type TransmitMessage = CastMessage<PackedMessage>;

#[derive(Clone)]
pub struct Broadcaster {
    msg_tx: Sender<TransmitMessage>,
}

impl Broadcaster {
    pub fn send(&mut self, msg: Message) {
        let packed_msg = PackedMessage { message: Some(msg) };

        // TODO: add a buffer to handle failure
        // or increase buffer in TransmissionProtocol ?
        let _ = self.msg_tx.try_send(CastMessage::All(packed_msg));
    }
}

pub struct Service {
    pub peer_manager: DefaultPeerManager,
    pub config: Config,

    pub(crate) msg_tx: Sender<TransmitMessage>,

    pub(crate) transmit_worker: ServiceWorker,
    pub(crate) service_worker: ServiceWorker,
}

impl Service {
    pub fn build() -> Builder {
        Builder::default()
    }

    pub async fn shutdown(self) {
        let error = |_| error!("Network: worker shutdown failure");
        let _ = await!(self.transmit_worker.shutdown()).map_err(error);
        let _ = await!(self.service_worker.shutdown()).map_err(error);
    }

    pub fn send(&mut self, msg: Message) {
        self.broadcaster().send(msg);
    }

    pub fn broadcaster(&self) -> Broadcaster {
        Broadcaster {
            msg_tx: self.msg_tx.clone(),
        }
    }
}

impl Service {
    // FIXME: cannot cleanly shutdown yet, wait for tentacle to implement
    // shutdown feature?
    pub(crate) fn service_worker(service_builder: ServiceBuilder, config: &Config) -> Task {
        let listening_address = config.listening_address();
        let bootstrap_addresses = config.bootstrap_addresses();
        let key_pair = config.key_pair();
        let mut service = service_builder.key_pair(key_pair).build(Service::handle());

        for addr in bootstrap_addresses {
            if let Err(err) = service.dial(addr.clone(), DialProtocol::All) {
                debug!("Network: dail {} failure: {}", addr, err);
            }
        }

        let _ = service.listen(listening_address);
        Box::new(service.for_each(|_| Ok(())))
    }

    pub(crate) fn transmit_worker(msg_rx: Receiver<PackedMessage>) -> Task {
        let worker = msg_rx.for_each(|msg| {
            // TODO: handling msg
            println!("{:?}", msg);
            Ok(())
        });

        Box::new(worker)
    }

    pub(crate) fn handle() -> Handle {
        Handle {}
    }
}

pub(crate) struct Handle {}

impl ServiceHandle for Handle {
    fn handle_error(&mut self, _: &mut ServiceContext, err: ServiceError) {
        error!("Network service error: {:?}", err)
    }

    fn handle_event(&mut self, _: &mut ServiceContext, _: ServiceEvent) {
        // no-op
    }
}
