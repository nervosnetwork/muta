use futures::channel::mpsc::UnboundedSender;
use protocol::Bytes;
use tentacle::context::ProtocolContextMutRef;
use tentacle::traits::SessionProtocol;

use super::message::ReceivedMessage;

pub struct TransmitterProtocol {
    data_tx:         UnboundedSender<ReceivedMessage>,
    latest_data_seq: u64,
}

impl TransmitterProtocol {
    pub fn new(data_tx: UnboundedSender<ReceivedMessage>) -> Self {
        TransmitterProtocol {
            data_tx,
            latest_data_seq: 0,
        }
    }
}

impl SessionProtocol for TransmitterProtocol {
    fn received(&mut self, ctx: ProtocolContextMutRef, data: Bytes) {
        let peer_id = match ctx.session.remote_pubkey.as_ref() {
            Some(pk) => pk.peer_id(),
            None => {
                // Dont care result here, connection/keeper will also handle this.
                let _ = ctx.disconnect(ctx.session.id);
                return;
            }
        };

        let recv_msg = ReceivedMessage {
            session_id: ctx.session.id,
            peer_id,
            data,
        };

        if self.data_tx.unbounded_send(recv_msg).is_err() {
            log::error!("network: transmitter: msg receiver dropped");
        }
    }
}
