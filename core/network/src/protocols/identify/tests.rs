use std::time::Duration;

use parking_lot::Mutex;
use tentacle::multiaddr::Multiaddr;
use tentacle::service::SessionType;
use tentacle::ProtocolId;
use futures_timer::Delay;
use protocol::Bytes;

use super::message;
use super::protocol::{
    ClientProcedure, Error, IdentifyProtocol, IdentifyProtocolContext, ServerProcedure, State,
    StateContext, MAX_MESSAGE_SIZE
};
use crate::test::mock::{ControlEvent, ProtocolContext};

const PROTOCOL_ID: usize = 2;
const SESSION_ID: usize = 2;

#[derive(Debug, Clone)]
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

    pub fn event(&self) -> Option<BehaviourEvent> {
        self.event.lock().clone()
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

#[tokio::test]
async fn should_wait_client_identity_for_inbound_connection() {
    let mut identify = IdentifyProtocol::new();
    let mut proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));
    match identify.state {
        State::ServerNegotiate {
            procedure: ServerProcedure::WaitIdentity,
            context,
        } => assert!(
            context.timeout_abort_handle.is_some(),
            "should set up wait timeout"
        ),
        _ => panic!("should enter failed state"),
    }
}

#[tokio::test]
async fn should_disconnect_if_wait_client_identity_timeout() {
    let mut identify = IdentifyProtocol::new();
    let mut proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));
    let mut context = match identify.state {
        State::ServerNegotiate {
            procedure: ServerProcedure::WaitIdentity,
            context,
        } => {
            assert!(context.timeout_abort_handle.is_some(), "should set up wait timeout");
            context
        },
        _ => panic!("should enter failed state"),
    };

    context.set_timeout("override wait identity", Duration::from_millis(300));
    Delay::new(Duration::from_millis(700)).await;

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}

#[tokio::test]
async fn should_register_opened_protocol() {
    let mut identify = IdentifyProtocol::new();
    let mut proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let peer_id = proto_context.session.remote_pubkey.as_ref().unwrap().peer_id();
    assert!(crate::protocols::OpenedProtocols::is_open(&peer_id, &PROTOCOL_ID.into()));
}

#[tokio::test]
async fn should_send_identity_to_server_for_outbound_connection() {
    let mut identify = IdentifyProtocol::new();
    let mut proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    match identify.state {
        State::ClientNegotiate {
            procedure: ClientProcedure::WaitAck,
            context,
        } => assert!(
            context.timeout_abort_handle.is_some(),
            "should set up wait timeout"
        ),
        _ => panic!("should enter failed state"),
    }

    match identify.behaviour.event() {
        Some(BehaviourEvent::SendIdentity) => (),
        _ => panic!("should send identity"),
    }
}

#[tokio::test]
async fn should_disconnect_if_wait_server_ack_timeout() {
    let mut identify = IdentifyProtocol::new();
    let mut proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let mut context = match identify.state {
        State::ClientNegotiate {
            procedure: ClientProcedure::WaitAck,
            context,
        } => {
            assert!(context.timeout_abort_handle.is_some(), "should set up wait timeout");
            context
        },
        _ => panic!("should enter failed state"),
    };

    match identify.behaviour.event() {
        Some(BehaviourEvent::SendIdentity) => (),
        _ => panic!("should send identity"),
    }

    context.set_timeout("override wait ack", Duration::from_millis(300));
    Delay::new(Duration::from_millis(700)).await;

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}

#[tokio::test]
async fn should_disconnect_if_exceed_max_message_size() {
    let mut identify = IdentifyProtocol::new();
    let mut proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    let msg = Bytes::from("a".repeat(MAX_MESSAGE_SIZE + 1));
    identify.on_received(&IdentifyProtocolContext(&proto_context), msg);

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}
