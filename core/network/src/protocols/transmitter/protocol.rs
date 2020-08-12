use std::time::Instant;

use futures::channel::mpsc::UnboundedSender;
use protocol::Bytes;
use tentacle::context::ProtocolContextMutRef;
use tentacle::traits::SessionProtocol;

use crate::common::ConnectedAddr;
use crate::peer_manager::PeerManagerHandle;

use super::message::{ReceivedMessage, SeqChunkMessage};
use super::{DATA_SEQ_TIMEOUT, MAX_CHUNK_SIZE};

pub struct TransmitterProtocol {
    data_tx:            UnboundedSender<ReceivedMessage>,
    peer_mgr:           PeerManagerHandle,
    data_buf:           Vec<u8>,
    current_data_seq:   u64,
    first_seq_bytes_at: Instant,
}

impl TransmitterProtocol {
    pub fn new(data_tx: UnboundedSender<ReceivedMessage>, peer_mgr: PeerManagerHandle) -> Self {
        TransmitterProtocol {
            data_tx,
            peer_mgr,
            data_buf: Vec::new(),
            current_data_seq: 0,
            first_seq_bytes_at: Instant::now(),
        }
    }
}

impl SessionProtocol for TransmitterProtocol {
    fn connected(&mut self, context: ProtocolContextMutRef, _version: &str) {
        if !self.peer_mgr.contains_session(context.session.id) {
            let _ = context.close_protocol(context.session.id, context.proto_id());
            return;
        }

        let peer_id = match context.session.remote_pubkey.as_ref() {
            Some(pubkey) => pubkey.peer_id(),
            None => {
                log::warn!("peer connection must be encrypted");
                let _ = context.disconnect(context.session.id);
                return;
            }
        };
        crate::protocols::OpenedProtocols::register(peer_id, context.proto_id());
    }

    fn received(&mut self, ctx: ProtocolContextMutRef, data: Bytes) {
        let peer_id = match ctx.session.remote_pubkey.as_ref() {
            Some(pk) => pk.peer_id(),
            None => {
                // Dont care result here, connection/keeper will also handle this.
                let _ = ctx.disconnect(ctx.session.id);
                return;
            }
        };
        let session_id = ctx.session.id;

        if data.len() < 10 {
            log::warn!("session {} data size < 10, drop it", session_id);
            return;
        }

        let SeqChunkMessage { seq, eof, data } = SeqChunkMessage::decode(data);
        log::debug!("recived seq {} eof {} data size {}", seq, eof, data.len());

        if data.len() > MAX_CHUNK_SIZE {
            log::warn!(
                "session {} data size > {}, drop it",
                session_id,
                MAX_CHUNK_SIZE
            );

            return;
        }

        if seq == self.current_data_seq {
            if self.first_seq_bytes_at.elapsed() > DATA_SEQ_TIMEOUT {
                log::warn!(
                    "session {} data seq {} timeout, drop it",
                    session_id,
                    self.current_data_seq
                );

                self.data_buf.clear();
                return;
            }

            self.data_buf.extend(data.as_ref());
            log::debug!("data buf size {}", self.data_buf.len());
        } else {
            log::debug!("new data seq {}", seq);

            self.current_data_seq = seq;
            self.data_buf.clear();
            self.data_buf.extend(data.as_ref());
            self.data_buf.shrink_to_fit();
            self.first_seq_bytes_at = Instant::now();
        }

        if !eof {
            return;
        }

        let data = std::mem::replace(&mut self.data_buf, Vec::new());
        log::debug!("final seq {} data size {}", seq, data.len());

        let recv_msg = ReceivedMessage {
            session_id,
            peer_id,
            data: Bytes::from(data),
        };

        let host = ConnectedAddr::from(&ctx.session.address).host;
        if self.data_tx.unbounded_send(recv_msg).is_err() {
            log::error!("network: transmitter: msg receiver dropped");
        } else {
            common_apm::metrics::network::NETWORK_RECEIVED_MESSAGE_IN_PROCESSING_GUAGE.inc();
            common_apm::metrics::network::NETWORK_RECEIVED_IP_MESSAGE_IN_PROCESSING_GUAGE_VEC
                .with_label_values(&[&host])
                .inc();
        }
    }
}
