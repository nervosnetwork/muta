use std::time::Instant;

use futures::channel::mpsc::UnboundedSender;
use protocol::Bytes;
use tentacle::context::ProtocolContextMutRef;
use tentacle::traits::SessionProtocol;

use super::message::{InternalMessage, ReceivedMessage};
use super::DATA_SEQ_TIMEOUT;

pub struct TransmitterProtocol {
    data_tx:            UnboundedSender<ReceivedMessage>,
    data_buf:           Vec<u8>,
    current_data_seq:   u64,
    first_seq_bytes_at: Instant,
}

impl TransmitterProtocol {
    pub fn new(data_tx: UnboundedSender<ReceivedMessage>) -> Self {
        TransmitterProtocol {
            data_tx,
            data_buf: Vec::new(),
            current_data_seq: 0,
            first_seq_bytes_at: Instant::now(),
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

        let InternalMessage { seq, eof, data } = InternalMessage::decode(data);
        if seq == self.current_data_seq {
            if self.first_seq_bytes_at.elapsed() > DATA_SEQ_TIMEOUT {
                log::warn!(
                    "session {} data seq {} timeout, drop it",
                    ctx.session.id,
                    self.current_data_seq
                );

                self.data_buf.clear();
                return;
            }

            self.data_buf.extend(data.as_ref());
        } else {
            self.data_buf.clear();
            self.data_buf.extend(data.as_ref());
            self.data_buf.shrink_to_fit();
            self.first_seq_bytes_at = Instant::now();
        }

        if !eof {
            return;
        }

        let data = std::mem::replace(&mut self.data_buf, Vec::new());
        let recv_msg = ReceivedMessage {
            session_id: ctx.session.id,
            peer_id,
            data: Bytes::from(data),
        };

        if self.data_tx.unbounded_send(recv_msg).is_err() {
            log::error!("network: transmitter: msg receiver dropped");
        }
    }
}
