use super::sync::Sync;

use core_network::{DiagnosticEvent, NetworkServiceHandle};
use protocol::{
    async_trait,
    traits::{Context, MessageHandler, PeerTrust, TrustFeedback},
};
use serde_derive::{Deserialize, Serialize};

use std::ops::Deref;

pub const GOSSIP_TRUST_NEW_INTERVAL: &str = "/gossip/diagnostic/trust_new_interval";
pub const GOSSIP_TRUST_TWIN_EVENT: &str = "/gossip/diagnostic/trust_twin_event";

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustNewIntervalReq(pub u8);

pub struct TrustNewIntervalHandler {
    pub sync:    Sync,
    pub network: NetworkServiceHandle,
}

impl TrustNewIntervalHandler {
    pub fn new(sync: Sync, network: NetworkServiceHandle) -> Self {
        TrustNewIntervalHandler { sync, network }
    }
}

#[async_trait]
impl MessageHandler for TrustNewIntervalHandler {
    type Message = TrustNewIntervalReq;

    async fn process(&self, ctx: Context, _msg: Self::Message) -> TrustFeedback {
        let session_id = ctx
            .get::<usize>("session_id")
            .cloned()
            .expect("impossible, session id not found");

        let report = self
            .network
            .diagnostic
            .new_trust_interval(session_id.into())
            .expect("failed to enter new trust interval");
        self.sync.emit(DiagnosticEvent::TrustNewInterval { report });

        TrustFeedback::Neutral
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TwinEvent {
    Good = 0,
    Bad = 1,
    Worse = 2,
    Both = 3,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustTwinEventReq(pub TwinEvent);

pub struct TrustTwinEventHandler(pub NetworkServiceHandle);

#[async_trait]
impl MessageHandler for TrustTwinEventHandler {
    type Message = TrustTwinEventReq;

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        match msg.0 {
            TwinEvent::Good => self.report(ctx.clone(), TrustFeedback::Good),
            TwinEvent::Bad => self.report(ctx.clone(), TrustFeedback::Bad("twin bad".to_owned())),
            TwinEvent::Worse => {
                self.report(ctx.clone(), TrustFeedback::Worse("twin worse".to_owned()))
            }
            TwinEvent::Both => {
                self.report(ctx.clone(), TrustFeedback::Good);
                self.report(ctx.clone(), TrustFeedback::Bad("twin bad".to_owned()));
            }
        }

        TrustFeedback::Neutral
    }
}

impl Deref for TrustTwinEventHandler {
    type Target = NetworkServiceHandle;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
