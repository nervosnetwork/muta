use protocol::traits::Priority;
use protocol::Bytes;
use tentacle::secio::PeerId;
use tentacle::service::TargetSession;
use tentacle::SessionId;

pub enum Recipient {
    Session(TargetSession),
    PeerId(Vec<PeerId>),
}

pub struct TransmitterMessage {
    pub recipient: Recipient,
    pub priority:  Priority,
    pub data:      Bytes,
}

pub struct ReceivedMessage {
    pub session_id: SessionId,
    pub peer_id:    PeerId,
    pub data:       Bytes,
}

struct InternalMessage {
    pub seq:  u64,
    pub data: Bytes,
}
