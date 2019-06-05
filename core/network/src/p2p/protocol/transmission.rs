use log::{debug, error, info};
use tentacle::context::{ProtocolContext, ProtocolContextMutRef};
use tentacle::service::{ProtocolHandle, ProtocolMeta};
use tentacle::{builder::MetaBuilder, bytes::Bytes, traits::ServiceProtocol};
use tentacle::{multiaddr::Multiaddr, ProtocolId, SessionId};

use common_channel::Sender;

use crate::context::Context;
use crate::Error;

macro_rules! log_format {
    () => {
        "protocol [transmission]: {}"
    };
    ($more:expr) => {
        concat!("protocol [transmission]: ", $more)
    };
}

/// Protocol name (handshake)
pub const PROTOCOL_NAME: &str = "transmission";

/// Protocol support versions
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.2"];

pub struct SessionMessage {
    pub id:   SessionId,
    pub addr: Multiaddr,
    pub body: Bytes,
}

pub struct TransmissionProtocol {
    id:   ProtocolId,
    ctx:  Context,
    r#in: Sender<SessionMessage>,
}

impl TransmissionProtocol {
    pub fn meta(ctx: Context, id: ProtocolId, r#in: Sender<SessionMessage>) -> ProtocolMeta {
        let service_handle =
            move || -> ProtocolHandle<Box<dyn ServiceProtocol + Send + 'static>> {
                let proto = TransmissionProtocol { ctx, id, r#in };
                ProtocolHandle::Callback(Box::new(proto))
            };

        MetaBuilder::default()
            .id(id)
            .name(name!(PROTOCOL_NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .service_handle(service_handle)
            .build()
    }
}

impl ServiceProtocol for TransmissionProtocol {
    fn init(&mut self, _: &mut ProtocolContext) {
        info!(log_format!("init: [id: {}]"), self.id);
    }

    fn connected(&mut self, context: ProtocolContextMutRef, _: &str) {
        debug!(log_format!("connected: {}"), context.session.id);
    }

    fn received(&mut self, ctx: ProtocolContextMutRef, data: Bytes) {
        debug!(log_format!("received len: {}"), data.len());

        let msg = SessionMessage {
            id:   ctx.session.id,
            addr: ctx.session.address.clone(),
            body: data,
        };

        if let Err(err) = self.r#in.try_send(msg) {
            if err.is_full() {
                // WARNNING: We drop received data. Msg should be consumed
                // immediately on the other side of channel. You should either
                // check that part or simply create a bigger capped channel.
                // But a bigger channle will lose more data after p2p reboot.
                //
                // TODO: notify network about this
                error!(log_format!(), "received: inbound channel is full");
            }
            if err.is_disconnected() {
                propagate_disconnection(&mut self.ctx, "received");
            }
        }
    }
}

fn propagate_disconnection(ctx: &mut Context, origin: &str) {
    if ctx.err_tx.try_send(Error::InboundDisconnected).is_err() {
        // WARNNING: This means fatal error, network is broken? need
        // to restart it from higher domain.
        error!(log_format!("{}: propagate disconnection failure"), origin);
    }
}
