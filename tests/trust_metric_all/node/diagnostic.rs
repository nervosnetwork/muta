use core_network::NetworkServiceHandle;
use protocol::{
    async_trait,
    traits::{Context, MessageHandler, PeerTrust, Priority, Rpc, TrustFeedback},
};
use serde_derive::{Deserialize, Serialize};

pub const RPC_TRUST_REPORT: &str = "/rpc_call/diagnostic/trust_report";
pub const RPC_RESP_TRUST_REPORT: &str = "/rpc_resp/diagnostic/trust_report";
pub const RPC_TRUST_NEW_INTERVAL: &str = "/rpc_call/diagnostic/trust_new_interval";
pub const RPC_RESP_TRUST_NEW_INTERVAL: &str = "/rpc_resp/diagnostic/trust_new_interval";
pub const RPC_TRUST_TWIN_EVENT: &str = "/rpc_call/diagnostic/trust_twin_event";
pub const RPC_RESP_TRUST_TWIN_EVENT: &str = "/rpc_resp/diagnostic/trust_twin_event";

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustReportReq(pub u8);

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustReport {
    pub worse_scalar_ratio: usize,
    pub good_events:        usize,
    pub bad_events:         usize,
    pub score:              u8,
}

pub struct TrustReportHandler(pub NetworkServiceHandle);

#[async_trait]
impl MessageHandler for TrustReportHandler {
    type Message = TrustReportReq;

    async fn process(&self, ctx: Context, _msg: Self::Message) -> TrustFeedback {
        let diagnostic = &self.0.diagnostic;

        let session_id = ctx
            .get::<usize>("session_id")
            .cloned()
            .expect("impossible, session id not found");

        let (good_events, bad_events) = diagnostic
            .session_trust_events(session_id.into())
            .expect("impossible, session doesn't have trust metric");

        let score = diagnostic
            .session_trust_score(session_id.into())
            .expect("impossible, session doesn't have trust metric");

        let report = TrustReport {
            worse_scalar_ratio: self.0.diagnostic.trust_metric_wrose_scalar_ratio(),
            good_events,
            bad_events,
            score,
        };

        self.0
            .response(ctx, RPC_RESP_TRUST_REPORT, Ok(report), Priority::High)
            .await
            .expect("failed to response trust report");

        TrustFeedback::Neutral
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustNewIntervalReq(pub u8);

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustNewIntervalResp(pub u8);

pub struct TrustNewIntervalHandler(pub NetworkServiceHandle);

#[async_trait]
impl MessageHandler for TrustNewIntervalHandler {
    type Message = TrustNewIntervalReq;

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        let echo_c0de: u8 = msg.0;
        let session_id = ctx
            .get::<usize>("session_id")
            .cloned()
            .expect("impossible, session id not found");

        self.0
            .diagnostic
            .session_trust_force_new_interval(session_id.into())
            .expect("failed to enter new trust interval");

        self.0
            .response(
                ctx,
                RPC_RESP_TRUST_NEW_INTERVAL,
                Ok(TrustNewIntervalResp(echo_c0de)),
                Priority::High,
            )
            .await
            .expect("failed to response trust new interval");

        TrustFeedback::Neutral
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TwinEvent {
    Good = 0,
    Bad = 1,
    Both = 2,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustTwinEventReq(pub TwinEvent);

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustTwinEventResp(pub TwinEvent);

pub struct TrustTwinEventHandler(pub NetworkServiceHandle);

#[async_trait]
impl MessageHandler for TrustTwinEventHandler {
    type Message = TrustTwinEventReq;

    async fn process(&self, ctx: Context, msg: Self::Message) -> TrustFeedback {
        match msg.0 {
            TwinEvent::Good => self.0.report(ctx.clone(), TrustFeedback::Good),
            TwinEvent::Bad => self
                .0
                .report(ctx.clone(), TrustFeedback::Bad("twin bad".to_owned())),
            TwinEvent::Both => {
                self.0.report(ctx.clone(), TrustFeedback::Good);
                self.0
                    .report(ctx.clone(), TrustFeedback::Bad("twin bad".to_owned()));
            }
        }

        self.0
            .response(
                ctx,
                RPC_RESP_TRUST_TWIN_EVENT,
                Ok(TrustTwinEventResp(msg.0)),
                Priority::High,
            )
            .await
            .expect("failed to response trust new interval");

        TrustFeedback::Neutral
    }
}
