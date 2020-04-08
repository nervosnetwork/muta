use protocol::{
    async_trait,
    traits::{Context, MessageHandler, TrustFeedback},
};
use serde_derive::{Deserialize, Serialize};

pub const GOSSIP_BLACKHOLE: &str = "/gossip/diagnostic/blackhole";

// Use Gossip::users_cast to call this endpoint, use result to check
// whether full node is connected.
#[derive(Debug, Serialize, Deserialize)]
pub struct BlackHoleMsg(pub usize);

pub struct BlackHoleHandler {}

#[async_trait]
impl MessageHandler for BlackHoleHandler {
    type Message = BlackHoleMsg;

    async fn process(&self, _ctx: Context, _msg: Self::Message) -> TrustFeedback {
        TrustFeedback::Neutral
    }
}
