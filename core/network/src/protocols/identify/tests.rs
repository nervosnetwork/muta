use parking_lot::Mutex;
use tentacle::multiaddr::Multiaddr;
use tentacle::service::SessionType;
use tentacle::ProtocolId;

use super::message;
use super::protocol::{Error, IdentifyProtocol, IdentifyProtocolContext, State, StateContext};
use crate::test::mock::{ControlEvent, ProtocolContext};

const PROTOCOL_ID: usize = 2;
const SESSION_ID: usize = 2;

#[derive(Debug)]
pub enum BehaviourEvent {
    SendIdentity,
    SendAck,
    ProcessListen,
    ProcessObserved,
    VerifyRemoteIdentity,
}

pub struct MockIdentifyBehaviour {
    event: Mutex<Option<BehaviourEvent>>,
}

impl MockIdentifyBehaviour {
    pub fn new() -> Self {
        MockIdentifyBehaviour {
            event: Mutex::new(None),
        }
    }

    pub fn send_identity(&self, _: &StateContext) {
        *self.event.lock() = Some(BehaviourEvent::SendIdentity)
    }

    pub fn send_ack(&self, _: &StateContext) {
        *self.event.lock() = Some(BehaviourEvent::SendAck)
    }

    pub fn process_listens(&self, _: &StateContext, _listen_addrs: Vec<Multiaddr>) {
        *self.event.lock() = Some(BehaviourEvent::ProcessListen)
    }

    pub fn process_observed(&self, _: &StateContext, _observed_addr: Multiaddr) {
        *self.event.lock() = Some(BehaviourEvent::ProcessObserved)
    }

    pub fn verify_remote_identity(&self, _identity: &message::Identity) -> Result<(), Error> {
        *self.event.lock() = Some(BehaviourEvent::VerifyRemoteIdentity);
        Ok(())
    }
}

#[test]
fn should_reject_unencrypted_connection() {
    let mut identify = IdentifyProtocol::new();
    let mut proto_context = ProtocolContext::make_no_encrypted(
        PROTOCOL_ID.into(),
        SESSION_ID.into(),
        SessionType::Inbound,
    );

    identify.on_connected(&IdentifyProtocolContext(&proto_context));
    match identify.state {
        State::FailedWithoutEncryption => (),
        _ => panic!("should enter failed state"),
    }
    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}
