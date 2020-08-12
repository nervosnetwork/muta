use tentacle::error::SendErrorKind;
use tentacle::service::{ServiceControl, TargetSession};
use tentacle::ProtocolId;

use protocol::traits::Priority;
use protocol::Bytes;

pub struct ProtocolMessage {
    pub protocol_id: ProtocolId,
    pub target:      TargetSession,
    pub data:        Bytes,
    pub priority:    Priority,
}

#[derive(Clone)]
pub struct ConnectionServiceControl {
    inner: ServiceControl,
}

impl ConnectionServiceControl {
    pub fn new(control: ServiceControl) -> Self {
        ConnectionServiceControl { inner: control }
    }

    pub fn send(&self, message: ProtocolMessage) -> Result<(), SendErrorKind> {
        let ProtocolMessage {
            target,
            protocol_id,
            data,
            ..
        } = message;

        match message.priority {
            Priority::High => self.inner.quick_filter_broadcast(target, protocol_id, data),
            Priority::Normal => self.inner.filter_broadcast(target, protocol_id, data),
        }
    }
}
