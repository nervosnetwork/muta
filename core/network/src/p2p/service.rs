use futures::future::ok;
use futures::prelude::Stream;
use futures::sync::mpsc::{Receiver, Sender};
use log::{debug, error};
use tentacle::builder::ServiceBuilder;
use tentacle::context::ServiceContext;
use tentacle::service::{DialProtocol, ServiceError, ServiceEvent};
use tentacle::traits::ServiceHandle;

use core_context::Context;
use core_p2p::{
    peer_manager::default_manager::{BorrowExt, PeerManagerHandle},
    peer_manager::PeerManager,
    transmission::{CastMessage, RecvMessage},
    DefaultPeerManager,
};

use crate::p2p::{Broadcaster, Builder, Config, Message, PackedMessage, ServiceWorker, Task};
use crate::reactor::{OutboundMessage, Reaction, Reactor, ReactorMessage};

type TransmitMessage = CastMessage<PackedMessage>;

pub struct Service {
    pub peer_manager: DefaultPeerManager,
    pub config:       Config,

    pub(crate) msg_tx: Sender<TransmitMessage>,

    pub(crate) transmit_worker: ServiceWorker,
    pub(crate) service_worker:  ServiceWorker,
}

impl Service {
    pub(crate) fn build<R>(reactor: R, outbound_rx: Receiver<OutboundMessage>) -> Builder<R>
    where
        R: Reactor<Input = ReactorMessage, Output = Reaction<ReactorMessage>> + Send + 'static,
    {
        Builder::new(reactor, outbound_rx)
    }

    pub async fn shutdown(self) {
        let error = |_| error!("Network: worker shutdown failure");
        let _ = self.transmit_worker.shutdown().map_err(error);
        let _ = self.service_worker.shutdown().map_err(error);
    }

    pub fn send(&mut self, ctx: Context, msg: Message) {
        self.broadcaster().send(ctx, msg);
    }

    pub fn broadcaster(&self) -> Broadcaster {
        Broadcaster::new(self.msg_tx.clone())
    }
}

impl Service {
    // FIXME: cannot cleanly shutdown yet, wait for tentacle to implement
    // shutdown feature?
    pub(crate) fn service_worker<M: PeerManager + 'static>(
        service_builder: ServiceBuilder,
        config: &Config,
        peer_manager: PeerManagerHandle<M>,
    ) -> Task {
        let listening_address = config.listening_address();
        let bootstrap_addresses = config.bootstrap_addresses();
        let key_pair = config.key_pair();
        let peer_mgr = peer_manager.borrow::<M>().clone();
        let mut service = service_builder
            .key_pair(key_pair)
            .build(Service::handle(peer_mgr));

        let listening_address = service.listen(listening_address).unwrap();
        log::info!("p2p listening {:?}", listening_address);

        for addr in bootstrap_addresses {
            if let Err(err) = service.dial(addr.clone(), DialProtocol::All) {
                debug!("Network: dail {} failure: {}", addr, err);
            }
        }

        Box::new(service.for_each(|_| Ok(())))
    }

    pub(crate) fn transmit_worker<R>(
        inbound_rx: Receiver<RecvMessage<PackedMessage>>,
        outbound_rx: Receiver<OutboundMessage>,
        broadcaster: Broadcaster,
        mut reactor: R,
    ) -> Task
    where
        R: Reactor<Input = ReactorMessage, Output = Reaction<ReactorMessage>> + Send + 'static,
    {
        let worker = outbound_rx
            .map(ReactorMessage::Outbound)
            .select(inbound_rx.map(ReactorMessage::Inbound))
            .for_each(move |msg| {
                tokio::spawn({
                    match reactor.react(broadcaster.clone(), msg) {
                        Reaction::Message(msg) => {
                            error!("network: drop unhandle msg: {:?}", msg);
                            Box::new(ok(())) // match `Done`
                        }
                        Reaction::Done(ret) => ret,
                    }
                });
                Ok(())
            });

        Box::new(worker)
    }

    pub(crate) fn handle<M: PeerManager>(peer_manager: M) -> Handle<M> {
        Handle {
            _peer_manager: peer_manager,
        }
    }
}

pub(crate) struct Handle<M> {
    _peer_manager: M,
}

impl<M: PeerManager> ServiceHandle for Handle<M> {
    fn handle_error(&mut self, _: &mut ServiceContext, err: ServiceError) {
        error!("Network service error: {:?}", err);

        // if let ServiceError::DialerError { address, error }
        // | ServiceError::ListenError { address, error } = err
        // {
        //     if let Error::RepeatedConnection(_) | Error::ConnectSelf = error {
        //         self.peer_manager.remove_masquer_addr(&address);
        //     }
        // }
    }

    fn handle_event(&mut self, _: &mut ServiceContext, _: ServiceEvent) {
        // no-op
    }
}
