use std::collections::HashMap;

use futures::channel::mpsc::{channel, Sender};
use futures::stream::StreamExt;
use futures::FutureExt;
use log::{debug, warn};
use tentacle::context::{ProtocolContext, ProtocolContextMutRef};
use tentacle::traits::ServiceProtocol;
use tentacle::SessionId;

use super::behaviour::{DiscoveryBehaviour, DiscoveryBehaviourHandle};
use super::substream::Substream;

pub struct DiscoveryProtocol {
    behaviour:         Option<DiscoveryBehaviour>,
    behaviour_handle:  DiscoveryBehaviourHandle,
    discovery_senders: HashMap<SessionId, Sender<Vec<u8>>>,
}

impl DiscoveryProtocol {
    pub fn new(behaviour: DiscoveryBehaviour) -> DiscoveryProtocol {
        let behaviour_handle = behaviour.handle();
        DiscoveryProtocol {
            behaviour: Some(behaviour),
            behaviour_handle,
            discovery_senders: HashMap::default(),
        }
    }
}

impl ServiceProtocol for DiscoveryProtocol {
    fn init(&mut self, context: &mut ProtocolContext) {
        debug!("protocol [discovery({})]: init", context.proto_id);

        let discovery_task = self
            .behaviour
            .take()
            .map(|mut behaviour| {
                debug!("Start discovery future_task");
                async move {
                    loop {
                        if behaviour.next().await.is_none() {
                            warn!("discovery stream shutdown");
                            break;
                        }
                    }
                }
                .boxed()
            })
            .unwrap();
        if context.future_task(discovery_task).is_err() {
            warn!("start discovery fail");
        };
    }

    fn connected(&mut self, context: ProtocolContextMutRef, _: &str) {
        let session = context.session;
        debug!(
            "protocol [discovery] open on session [{}], address: [{}], type: [{:?}]",
            session.id, session.address, session.ty
        );

        if !self.behaviour_handle.contains_session(session.id) {
            let _ = context.close_protocol(session.id, context.proto_id());
            return;
        }

        let (sender, receiver) = channel(8);
        self.discovery_senders.insert(session.id, sender);
        let substream = Substream::new(context, receiver);
        match self.behaviour_handle.substream_sender.try_send(substream) {
            Ok(_) => {
                debug!("Send substream success");
            }
            Err(err) => {
                // TODO: handle channel is full (wait for poll API?)
                warn!("Send substream failed : {:?}", err);
            }
        }
    }

    fn disconnected(&mut self, context: ProtocolContextMutRef) {
        self.discovery_senders.remove(&context.session.id);
        debug!(
            "protocol [discovery] close on session [{}]",
            context.session.id
        );
    }

    fn received(&mut self, context: ProtocolContextMutRef, data: bytes::Bytes) {
        debug!("[received message]: length={}", data.len());

        if let Some(ref mut sender) = self.discovery_senders.get_mut(&context.session.id) {
            // TODO: handle channel is full (wait for poll API?)
            if let Err(err) = sender.try_send(data.to_vec()) {
                if err.is_full() {
                    warn!("channel is full");
                } else if err.is_disconnected() {
                    warn!("channel is disconnected");
                } else {
                    warn!("other channel error: {:?}", err);
                }
            }
        }
    }
}
