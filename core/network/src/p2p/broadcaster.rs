use futures::sync::mpsc::Sender;
use log::{debug, error};

use core_context::{CommonValue, Context};
use core_p2p::transmission::CastMessage;

use crate::p2p::{Message, PackedMessage};

type TransmitMessage = CastMessage<PackedMessage>;

#[derive(Clone)]
pub struct Broadcaster {
    msg_tx: Sender<TransmitMessage>,
}

impl Broadcaster {
    pub(crate) fn new(msg_tx: Sender<TransmitMessage>) -> Self {
        Broadcaster { msg_tx }
    }

    // TODO: add a buffer to handle failure or increase buffer in
    // TransmissionProtocol ?
    pub fn send(&mut self, ctx: Context, msg: Message) {
        let packed_msg = PackedMessage {
            message: Some(msg.clone()),
        };

        let cast_msg = {
            if let Some(session_id) = ctx.p2p_session_id() {
                debug!("network: broadcaster uni message: {:?}", packed_msg);

                CastMessage::Uni {
                    session_id,
                    msg: packed_msg,
                }
            } else {
                CastMessage::All(packed_msg)
            }
        };

        if let Err(err) = self.msg_tx.try_send(cast_msg) {
            error!("network: broadcaster message failure: {:?}", err);
        }
    }
}
