use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{debug, error, trace, warn};
use parking_lot::RwLock;
use prost::Message;
use tentacle::context::{ProtocolContext, ProtocolContextMutRef, SessionContext};
use tentacle::multiaddr::{Multiaddr, Protocol};
use tentacle::secio::PeerId;
use tentacle::traits::ServiceProtocol;
use tentacle::SessionId;

use super::behaviour::{IdentifyBehaviour, Misbehavior, MAX_ADDRS};
use super::common::reachable;
use super::identification::{Identification, WaitIdentification};
use super::message::IdentifyMessage;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(8);
const CHECK_TIMEOUT_INTERVAL: Duration = Duration::from_secs(1);
const CHECK_TIMEOUT_TOKEN: u64 = 100;

pub struct RemoteInfo {
    pub peer_id:        PeerId,
    pub session:        SessionContext,
    pub connected_at:   Instant,
    pub timeout:        Duration,
    pub listen_addrs:   Option<Vec<Multiaddr>>,
    pub observed_addr:  Option<Multiaddr>,
    pub identification: Identification,
}

impl RemoteInfo {
    pub fn new(peer_id: PeerId, session: SessionContext, timeout: Duration) -> RemoteInfo {
        RemoteInfo {
            peer_id,
            session,
            connected_at: Instant::now(),
            timeout,
            listen_addrs: None,
            observed_addr: None,
            identification: Identification::new(),
        }
    }
}

#[derive(Clone)]
pub struct IdentifyProtocol {
    remote_infos: Arc<RwLock<HashMap<SessionId, RemoteInfo>>>,
    behaviour:    Arc<IdentifyBehaviour>,
}

impl IdentifyProtocol {
    pub fn new(behaviour: IdentifyBehaviour) -> Self {
        IdentifyProtocol {
            remote_infos: Default::default(),
            behaviour:    Arc::new(behaviour),
        }
    }

    pub fn wait(&self, context: &ProtocolContextMutRef) -> Result<WaitIdentification, ()> {
        if self.insert_info_if_new(context).is_err() {
            return Err(());
        }

        match self.remote_infos.read().get(&context.session.id) {
            Some(remote_info) => Ok(remote_info.identification.wait()),
            None => Err(()),
        }
    }

    fn insert_info_if_new(&self, context: &ProtocolContextMutRef) -> Result<(), ()> {
        let session = context.session;
        {
            if self.remote_infos.read().get(&session.id).is_some() {
                return Ok(());
            }
        }

        let remote_peer_id = match &session.remote_pubkey {
            Some(pubkey) => pubkey.peer_id(),
            None => {
                error!("IdentifyProtocol require secio enabled!");
                let _ = context.disconnect(session.id);
                return Err(());
            }
        };

        trace!("IdentifyProtocol connected from {:?}", remote_peer_id);
        let remote_info = RemoteInfo::new(remote_peer_id, session.clone(), DEFAULT_TIMEOUT);
        {
            self.remote_infos.write().insert(session.id, remote_info);
        }

        Ok(())
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
        if self.insert_info_if_new(&context).is_err() {
            return;
        }

        let listen_addrs: Vec<Multiaddr> = self
            .behaviour
            .local_listen_addrs()
            .into_iter()
            .filter(reachable)
            .take(MAX_ADDRS)
            .collect();

        let observed_addr = context
            .session
            .address
            .iter()
            .filter(|proto| match proto {
                Protocol::P2P(_) => false,
                _ => true,
            })
            .collect::<Multiaddr>();

        let identify = self.behaviour.identify();
        let msg = match IdentifyMessage::new(listen_addrs, observed_addr, identify).into_bytes() {
            Ok(msg) => msg,
            Err(err) => {
                warn!("encode {}", err);
                return;
            }
        };

        if let Err(err) = context.quick_send_message(msg) {
            warn!("quick send message {}", err);
        }
    }

    fn disconnected(&mut self, context: ProtocolContextMutRef) {
        let info = {
            let mut infos = self.remote_infos.write();
            infos.remove(&context.session.id)
        };

        trace!(
            "IdentifyProtocol disconnected from {:?}",
            info.map(|i| i.peer_id)
        );
    }

    fn received(&mut self, context: ProtocolContextMutRef, data: bytes::Bytes) {
        let session = context.session;

        match IdentifyMessage::decode(data) {
            Ok(message) => {
                let mut infos = self.remote_infos.write();

                let mut remote_info = infos.get_mut(&session.id).expect("RemoteInfo must exists");

                let behaviour = &mut self.behaviour;

                // Need to interrupt processing, avoid pollution
                if behaviour
                    .received_identify(&mut remote_info, message.identify.as_bytes())
                    .is_disconnect()
                    || behaviour
                        .process_listens(&mut remote_info, message.listen_addrs())
                        .is_disconnect()
                    || behaviour
                        .process_observed(&mut remote_info, message.observed_addr())
                        .is_disconnect()
                {
                    let _ = context.disconnect(session.id);
                }
            }
            Err(_) => {
                let infos = self.remote_infos.read();
                let remote_info = infos.get(&session.id).expect("RemoteInfo must exists");

                warn!(
                    "IdentifyProtocol received invalid data from {:?}",
                    remote_info.peer_id
                );

                if self
                    .behaviour
                    .misbehave(&remote_info.peer_id, Misbehavior::InvalidData)
                    .is_disconnect()
                {
                    let _ = context.disconnect(session.id);
                }
            }
        }
    }

    fn notify(&mut self, context: &mut ProtocolContext, _token: u64) {
        let now = Instant::now();

        for (session_id, info) in self.remote_infos.read().iter() {
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
