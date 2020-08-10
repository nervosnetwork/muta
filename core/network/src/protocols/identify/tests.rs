use std::time::Duration;

use futures_timer::Delay;
use parking_lot::Mutex;
use protocol::Bytes;
use tentacle::multiaddr::Multiaddr;
use tentacle::service::{SessionType, TargetProtocol};

use super::message;
use super::protocol::{
    ClientProcedure, Error, IdentifyProtocol, IdentifyProtocolContext, ServerProcedure, State,
    StateContext, MAX_MESSAGE_SIZE,
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
    event:                Mutex<Option<BehaviourEvent>>,
    skip_chain_id_verify: Mutex<bool>,
}

impl MockIdentifyBehaviour {
    pub fn new() -> Self {
        MockIdentifyBehaviour {
            event:                Mutex::new(None),
            skip_chain_id_verify: Mutex::new(true),
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
        {
            *self.event.lock() = Some(BehaviourEvent::VerifyRemoteIdentity);
        }

        if *self.skip_chain_id_verify.lock() {
            Ok(())
        } else {
            Err(Error::WrongChainId)
        }
    }

    pub fn skip_chain_id_verify(&self, result: bool) {
        *self.skip_chain_id_verify.lock() = result;
    }
}

#[test]
fn should_reject_unencrypted_connection() {
    let mut identify = IdentifyProtocol::new();
    let proto_context = ProtocolContext::make_no_encrypted(
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
    let proto_context =
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
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));
    let mut context = match identify.state {
        State::ServerNegotiate {
            procedure: ServerProcedure::WaitIdentity,
            context,
        } => {
            assert!(
                context.timeout_abort_handle.is_some(),
                "should set up wait timeout"
            );
            context
        }
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
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let peer_id = proto_context
        .session
        .remote_pubkey
        .as_ref()
        .unwrap()
        .peer_id();
    assert!(crate::protocols::OpenedProtocols::is_open(
        &peer_id,
        &PROTOCOL_ID.into()
    ));
}

#[tokio::test]
async fn should_send_identity_to_server_for_outbound_connection() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
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
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let mut context = match identify.state {
        State::ClientNegotiate {
            procedure: ClientProcedure::WaitAck,
            context,
        } => {
            assert!(
                context.timeout_abort_handle.is_some(),
                "should set up wait timeout"
            );
            context
        }
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
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    let msg = Bytes::from("a".repeat(MAX_MESSAGE_SIZE + 1));
    identify.on_received(&IdentifyProtocolContext(&proto_context), msg);

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}

#[tokio::test]
async fn should_send_ack_if_identity_is_valid_on_server_side() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let identity = message::Identity::mock_valid().into_bytes().unwrap();
    identify.behaviour.skip_chain_id_verify(true);
    identify.on_received(&IdentifyProtocolContext(&proto_context), identity);

    match identify.state {
        State::ServerNegotiate {
            procedure: ServerProcedure::WaitOpenProtocols,
            context,
        } => assert!(
            context.timeout_abort_handle.is_some(),
            "should set up wait open protocols timeout"
        ),
        _ => panic!("should enter wait open protocols state"),
    }

    match identify.behaviour.event() {
        Some(BehaviourEvent::SendAck) => (),
        _ => panic!("should send ack"),
    }
}

#[tokio::test]
async fn should_disconnect_if_client_open_protocols_timeout() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let identity = message::Identity::mock_valid().into_bytes().unwrap();
    identify.behaviour.skip_chain_id_verify(true);
    identify.on_received(&IdentifyProtocolContext(&proto_context), identity);

    let mut context = match identify.state {
        State::ServerNegotiate {
            procedure: ServerProcedure::WaitOpenProtocols,
            context,
        } => {
            assert!(
                context.timeout_abort_handle.is_some(),
                "should set up wait open protocols timeout"
            );
            context
        }
        _ => panic!("should enter wait open protocols state"),
    };

    match identify.behaviour.event() {
        Some(BehaviourEvent::SendAck) => (),
        _ => panic!("should send ack"),
    }

    context.set_timeout("override wait open protocols", Duration::from_millis(300));
    Delay::new(Duration::from_millis(700)).await;

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}

#[tokio::test]
async fn should_disconnect_if_client_send_undecodeable_identity() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let msg = Bytes::from("a");
    identify.on_received(&IdentifyProtocolContext(&proto_context), msg);

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }

    match identify.state {
        State::ServerNegotiate {
            procedure: ServerProcedure::Failed,
            ..
        } => (),
        _ => panic!("should enter failed state"),
    }
}

#[tokio::test]
async fn should_disconnect_if_client_send_invalid_identity() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let msg = message::Identity::mock_invalid().into_bytes().unwrap();
    identify.on_received(&IdentifyProtocolContext(&proto_context), msg);

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }

    match identify.state {
        State::ServerNegotiate {
            procedure: ServerProcedure::Failed,
            ..
        } => (),
        _ => panic!("should enter failed state"),
    }
}

#[tokio::test]
async fn should_disconnect_if_client_send_different_chain_id() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let msg = message::Identity::mock_valid().into_bytes().unwrap();
    identify.behaviour.skip_chain_id_verify(false);
    identify.on_received(&IdentifyProtocolContext(&proto_context), msg);

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }

    match identify.state {
        State::ServerNegotiate {
            procedure: ServerProcedure::Failed,
            ..
        } => (),
        _ => panic!("should enter failed state"),
    }
}

#[tokio::test]
async fn should_disconnect_if_client_send_data_during_open_protocols() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let identity = message::Identity::mock_valid().into_bytes().unwrap();
    identify.behaviour.skip_chain_id_verify(true);
    identify.on_received(&IdentifyProtocolContext(&proto_context), identity);

    match &identify.state {
        State::ServerNegotiate {
            procedure: ServerProcedure::WaitOpenProtocols,
            context,
        } => assert!(
            context.timeout_abort_handle.is_some(),
            "should set up wait open protocols timeout"
        ),
        _ => panic!("should enter wait open protocols state"),
    }

    match identify.behaviour.event() {
        Some(BehaviourEvent::SendAck) => (),
        _ => panic!("should send ack"),
    }

    identify.on_received(
        &IdentifyProtocolContext(&proto_context),
        Bytes::from_static(b"test"),
    );

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}

#[tokio::test]
async fn should_open_protocols_after_receive_valid_ack_from_server() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let ack = message::Acknowledge::mock_valid().into_bytes().unwrap();
    identify.on_received(&IdentifyProtocolContext(&proto_context), ack);

    match identify.state {
        State::ClientNegotiate {
            procedure: ClientProcedure::OpenOtherProtocols,
            ..
        } => (),
        _ => panic!("should enter wait open protocols state"),
    }

    match proto_context.control().event() {
        Some(ControlEvent::OpenProtocols {
            session_id,
            target_proto,
        }) if session_id == SESSION_ID.into() && target_proto == TargetProtocol::All => (),
        _ => panic!("should open protocols"),
    }
}

#[tokio::test]
async fn should_disconnect_if_server_send_undecodeable_ack() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    identify.on_received(
        &IdentifyProtocolContext(&proto_context),
        Bytes::from_static(b"xxx"),
    );

    match identify.state {
        State::ClientNegotiate {
            procedure: ClientProcedure::Failed,
            ..
        } => (),
        _ => panic!("should enter failed state"),
    }

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}

#[tokio::test]
async fn should_disconnect_if_server_send_invalid_ack() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let ack = message::Acknowledge::mock_invalid().into_bytes().unwrap();
    identify.on_received(&IdentifyProtocolContext(&proto_context), ack);

    match identify.state {
        State::ClientNegotiate {
            procedure: ClientProcedure::Failed,
            ..
        } => (),
        _ => panic!("should enter failed state"),
    }

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}

#[tokio::test]
async fn should_disconnect_if_server_send_data_during_open_protocols() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    identify.on_connected(&IdentifyProtocolContext(&proto_context));

    let ack = message::Acknowledge::mock_valid().into_bytes().unwrap();
    identify.on_received(&IdentifyProtocolContext(&proto_context), ack);

    match &identify.state {
        State::ClientNegotiate {
            procedure: ClientProcedure::OpenOtherProtocols,
            ..
        } => (),
        _ => panic!("should enter wait open protocols state"),
    }

    match proto_context.control().event() {
        Some(ControlEvent::OpenProtocols {
            session_id,
            target_proto,
        }) if session_id == SESSION_ID.into() && target_proto == TargetProtocol::All => (),
        _ => panic!("should open protocols"),
    }

    identify.on_received(
        &IdentifyProtocolContext(&proto_context),
        Bytes::from_static(b"test"),
    );

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}

#[tokio::test]
async fn should_disconnect_if_either_send_data_no_in_negotiate_procedure() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    identify.on_received(
        &IdentifyProtocolContext(&proto_context),
        Bytes::from_static(b"test"),
    );

    match proto_context.control().event() {
        Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
        _ => panic!("should disconnect"),
    }
}

#[tokio::test]
async fn should_wake_wait_identification_after_call_finish_identify() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Inbound);

    let peer_id = proto_context
        .session
        .remote_pubkey
        .as_ref()
        .unwrap()
        .peer_id();

    let wait_fut = IdentifyProtocol::wait(peer_id);

    tokio::spawn(async move {
        identify.on_connected(&IdentifyProtocolContext(&proto_context));

        let identity = message::Identity::mock_valid().into_bytes().unwrap();
        identify.behaviour.skip_chain_id_verify(true);
        identify.on_received(&IdentifyProtocolContext(&proto_context), identity);

        match identify.state {
            State::ServerNegotiate {
                procedure: ServerProcedure::WaitOpenProtocols,
                context,
            } => assert!(
                context.timeout_abort_handle.is_some(),
                "should set up wait open protocols timeout"
            ),
            _ => panic!("should enter wait open protocols state"),
        }

        match identify.behaviour.event() {
            Some(BehaviourEvent::SendAck) => (),
            _ => panic!("should send ack"),
        }
    });

    match wait_fut.await {
        Ok(()) => (),
        Err(_) => panic!("should be ok if pass identify"),
    }
}

#[tokio::test]
async fn should_pass_error_to_wait_identification_result_if_failed_identify() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    let peer_id = proto_context
        .session
        .remote_pubkey
        .as_ref()
        .unwrap()
        .peer_id();

    let wait_fut = IdentifyProtocol::wait(peer_id);

    tokio::spawn(async move {
        identify.on_connected(&IdentifyProtocolContext(&proto_context));

        identify.on_received(
            &IdentifyProtocolContext(&proto_context),
            Bytes::from_static(b"xxx"),
        );

        match identify.state {
            State::ClientNegotiate {
                procedure: ClientProcedure::Failed,
                ..
            } => (),
            _ => panic!("should enter failed state"),
        }

        match proto_context.control().event() {
            Some(ControlEvent::Disconnect { session_id }) if session_id == SESSION_ID.into() => (),
            _ => panic!("should disconnect"),
        }
    });

    match wait_fut.await {
        Err(Error::DecodeAckFailed) => (),
        _ => panic!("should pass decode failed error"),
    }
}

#[tokio::test]
async fn should_pass_disconnected_to_wait_identification_result_if_still_wait_identify_but_disconnected() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    let peer_id = proto_context
        .session
        .remote_pubkey
        .as_ref()
        .unwrap()
        .peer_id();

    let wait_fut = IdentifyProtocol::wait(peer_id);

    tokio::spawn(async move {
        identify.on_connected(&IdentifyProtocolContext(&proto_context));
        identify.on_disconnected(&IdentifyProtocolContext(&proto_context));
    });

    match wait_fut.await {
        Err(Error::Disconnected) => (),
        _ => panic!("should pass disconnected error"),
    }
}

#[tokio::test]
async fn should_remove_from_opened_protocols_after_disconnect() {
    let mut identify = IdentifyProtocol::new();
    let proto_context =
        ProtocolContext::make(PROTOCOL_ID.into(), SESSION_ID.into(), SessionType::Outbound);

    let peer_id = proto_context
        .session
        .remote_pubkey
        .as_ref()
        .unwrap()
        .peer_id();

    identify.on_connected(&IdentifyProtocolContext(&proto_context));
    identify.on_disconnected(&IdentifyProtocolContext(&proto_context));

    assert_eq!(crate::protocols::OpenedProtocols::is_open(
        &peer_id,
        &PROTOCOL_ID.into()
    ), false);
}
