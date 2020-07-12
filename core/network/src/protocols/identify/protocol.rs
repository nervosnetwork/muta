use super::{
    behaviour::{IdentifyBehaviour, Misbehavior, RemoteInfo, MAX_ADDRS},
    message::IdentifyMessage,
};

use log::{debug, error, trace, warn};
use tentacle::{
    context::{ProtocolContext, ProtocolContextMutRef},
    multiaddr::{Multiaddr, Protocol},
    traits::ServiceProtocol,
    utils::{is_reachable, multiaddr_to_socketaddr},
};

use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT: u64 = 8;
const CHECK_TIMEOUT_INTERVAL: u64 = 1;
const CHECK_TIMEOUT_TOKEN: u64 = 100;

pub struct IdentifyProtocol {
    secio_enabled: bool,
    behaviour:     IdentifyBehaviour,
}

impl IdentifyProtocol {
    pub fn new(behaviour: IdentifyBehaviour) -> Self {
        IdentifyProtocol {
            secio_enabled: true,
            behaviour,
        }
    }
}

impl ServiceProtocol for IdentifyProtocol {
    fn init(&mut self, context: &mut ProtocolContext) {
        let proto_id = context.proto_id;
        if context
            .set_service_notify(
                proto_id,
                Duration::from_secs(CHECK_TIMEOUT_INTERVAL),
                CHECK_TIMEOUT_TOKEN,
            )
            .is_err()
        {
            warn!("identify start fail")
        }
    }

    fn connected(&mut self, context: ProtocolContextMutRef, _version: &str) {
        let session = context.session;
        if session.remote_pubkey.is_none() {
            error!("IdentifyProtocol require secio enabled!");
            let _ = context.disconnect(session.id);
            self.secio_enabled = false;
            return;
        }

        let remote_info = RemoteInfo::new(session.clone(), Duration::from_secs(DEFAULT_TIMEOUT));
        trace!("IdentifyProtocol sconnected from {:?}", remote_info.peer_id);
        self.behaviour.remote_infos.insert(session.id, remote_info);

        let listen_addrs: Vec<Multiaddr> = self
            .behaviour
            .callback
            .local_listen_addrs()
            .iter()
            .filter(|addr| {
                multiaddr_to_socketaddr(addr)
                    .map(|socket_addr| {
                        !self.behaviour.global_ip_only() || is_reachable(socket_addr.ip())
                    })
                    .unwrap_or(false)
            })
            .take(MAX_ADDRS)
            .cloned()
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
        if self.secio_enabled {
            let info = self
                .behaviour
                .remote_infos
                .remove(&context.session.id)
                .expect("RemoteInfo must exists");
            trace!("IdentifyProtocol disconnected from {:?}", info.peer_id);
        }
    }

    fn received(&mut self, mut context: ProtocolContextMutRef, data: bytes::Bytes) {
        if !self.secio_enabled {
            return;
        }

        let session = context.session;

        match IdentifyMessage::decode(&data) {
            Some(message) => {
                // Need to interrupt processing, avoid pollution
                if self
                    .behaviour
                    .callback
                    .received_identify(&mut context, message.identify)
                    .is_disconnect()
                    || self
                        .behaviour
                        .process_listens(&mut context, message.listen_addrs)
                        .is_disconnect()
                    || self
                        .behaviour
                        .process_observed(&mut context, message.observed_addr)
                        .is_disconnect()
                {
                    let _ = context.disconnect(session.id);
                }
            }
            None => {
                let info = self
                    .behaviour
                    .remote_infos
                    .get(&session.id)
                    .expect("RemoteInfo must exists");
                debug!(
                    "IdentifyProtocol received invalid data from {:?}",
                    info.peer_id
                );
                if self
                    .behaviour
                    .callback
                    .misbehave(&info.peer_id, Misbehavior::InvalidData)
                    .is_disconnect()
                {
                    let _ = context.disconnect(session.id);
                }
            }
        }
    }

    fn notify(&mut self, context: &mut ProtocolContext, _token: u64) {
        if !self.secio_enabled {
            return;
        }

        let now = Instant::now();
        for (session_id, info) in &self.behaviour.remote_infos {
            if (info.listen_addrs.is_none() || info.observed_addr.is_none())
                && (info.connected_at + info.timeout) <= now
            {
                debug!("{:?} receive identify message timeout", info.peer_id);
                if self
                    .behaviour
                    .callback
                    .misbehave(&info.peer_id, Misbehavior::Timeout)
                    .is_disconnect()
                {
                    let _ = context.disconnect(*session_id);
                }
            }
        }
    }
}
