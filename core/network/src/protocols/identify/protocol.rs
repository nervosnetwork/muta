use super::{
    behaviour::{IdentifyBehaviour, Misbehavior, RemoteInfo, MAX_ADDRS},
    common::reachable,
    message::IdentifyMessage,
};

use log::{debug, error, trace, warn};
use tentacle::{
    context::{ProtocolContext, ProtocolContextMutRef},
    multiaddr::{Multiaddr, Protocol},
    traits::ServiceProtocol,
    SessionId,
};

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(8);
const CHECK_TIMEOUT_INTERVAL: Duration = Duration::from_secs(1);
const CHECK_TIMEOUT_TOKEN: u64 = 100;

pub struct IdentifyProtocol {
    remote_infos: HashMap<SessionId, RemoteInfo>,
    behaviour:    IdentifyBehaviour,
}

impl IdentifyProtocol {
    pub fn new(behaviour: IdentifyBehaviour) -> Self {
        IdentifyProtocol {
            remote_infos: HashMap::new(),
            behaviour,
        }
    }
}

impl ServiceProtocol for IdentifyProtocol {
    fn init(&mut self, context: &mut ProtocolContext) {
        let proto_id = context.proto_id;

        if let Err(e) =
            context.set_service_notify(proto_id, CHECK_TIMEOUT_INTERVAL, CHECK_TIMEOUT_TOKEN)
        {
            warn!("identify start fail {}", e);
        }
    }

    fn connected(&mut self, context: ProtocolContextMutRef, _version: &str) {
        let session = context.session;
        let remote_peer_id = match &session.remote_pubkey {
            Some(pubkey) => pubkey.peer_id(),
            None => {
                error!("IdentifyProtocol require secio enabled!");
                let _ = context.disconnect(session.id);
                return;
            }
        };

        trace!("IdentifyProtocol connected from {:?}", remote_peer_id);
        let remote_info = RemoteInfo::new(remote_peer_id, session.clone(), DEFAULT_TIMEOUT);
        self.remote_infos.insert(session.id, remote_info);

        let listen_addrs: Vec<Multiaddr> = self
            .behaviour
            .local_listen_addrs()
            .into_iter()
            .filter(reachable)
            .take(MAX_ADDRS)
            .collect();

        let observed_addr = session
            .address
            .iter()
            .filter(|proto| match proto {
                Protocol::P2P(_) => false,
                _ => true,
            })
            .collect::<Multiaddr>();

        let identify = self.behaviour.identify();
        let data = IdentifyMessage::new(listen_addrs, observed_addr, identify).encode();
        let _ = context.quick_send_message(data);
    }

    fn disconnected(&mut self, context: ProtocolContextMutRef) {
        let info = self
            .remote_infos
            .remove(&context.session.id)
            .expect("RemoteInfo must exists");
        trace!("IdentifyProtocol disconnected from {:?}", info.peer_id);
    }

    fn received(&mut self, mut context: ProtocolContextMutRef, data: bytes::Bytes) {
        let session = context.session;

        match IdentifyMessage::decode(&data) {
            Some(message) => {
                let mut remote_info = self
                    .remote_infos
                    .get_mut(&context.session.id)
                    .expect("RemoteInfo must exists");
                let behaviour = &mut self.behaviour;

                // Need to interrupt processing, avoid pollution
                if behaviour
                    .received_identify(&mut context, message.identify)
                    .is_disconnect()
                    || behaviour
                        .process_listens(&mut remote_info, message.listen_addrs)
                        .is_disconnect()
                    || behaviour
                        .process_observed(&mut remote_info, message.observed_addr)
                        .is_disconnect()
                {
                    let _ = context.disconnect(session.id);
                }
            }
            None => {
                let info = self
                    .remote_infos
                    .get(&session.id)
                    .expect("RemoteInfo must exists");
                debug!(
                    "IdentifyProtocol received invalid data from {:?}",
                    info.peer_id
                );
                if self
                    .behaviour
                    .misbehave(&info.peer_id, Misbehavior::InvalidData)
                    .is_disconnect()
                {
                    let _ = context.disconnect(session.id);
                }
            }
        }
    }

    fn notify(&mut self, context: &mut ProtocolContext, _token: u64) {
        let now = Instant::now();

        for (session_id, info) in &self.remote_infos {
            if (info.listen_addrs.is_none() || info.observed_addr.is_none())
                && (info.connected_at + info.timeout) <= now
            {
                debug!("{:?} receive identify message timeout", info.peer_id);
                if self
                    .behaviour
                    .misbehave(&info.peer_id, Misbehavior::Timeout)
                    .is_disconnect()
                {
                    let _ = context.disconnect(*session_id);
                }
            }
        }
    }
}
