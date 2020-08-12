use std::time::Instant;

use protocol::Bytes;
use tentacle::context::ProtocolContextMutRef;
use tentacle::traits::SessionProtocol;

use crate::compression::Snappy;
use crate::peer_manager::PeerManagerHandle;
use crate::reactor::{MessageRouter, RemotePeer};

use super::message::{ReceivedMessage, SeqChunkMessage};
use super::{DATA_SEQ_TIMEOUT, MAX_CHUNK_SIZE};

pub struct TransmitterProtocol {
    router:             MessageRouter<Snappy>,
    peer_mgr:           PeerManagerHandle,
    data_buf:           Vec<u8>,
    current_data_seq:   u64,
    first_seq_bytes_at: Instant,
}

impl TransmitterProtocol {
    pub fn new(router: MessageRouter<Snappy>, peer_mgr: PeerManagerHandle) -> Self {
        TransmitterProtocol {
            router,
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

        // Seq u64 takes 8 bytes, and eof bool take 1 byte, so a valid data length
        // must be bigger or equal than 10.
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

        let remote_peer = match RemotePeer::from_proto_context(&ctx) {
            Ok(peer) => peer,
            Err(_err) => {
                log::warn!("received data from unencrypted peer, impossible, drop it");
                return;
            }
        };

        let recv_msg = ReceivedMessage {
            session_id,
            peer_id,
            data: Bytes::from(data),
        };

        let host = remote_peer.connected_addr.host.to_owned();
        let route_fut = self.router.route_message(remote_peer.clone(), recv_msg);
        tokio::spawn(async move {
            common_apm::metrics::network::NETWORK_RECEIVED_MESSAGE_IN_PROCESSING_GUAGE.inc();
            common_apm::metrics::network::NETWORK_RECEIVED_IP_MESSAGE_IN_PROCESSING_GUAGE_VEC
                .with_label_values(&[&host])
                .inc();

            if let Err(err) = route_fut.await {
                log::warn!("route {} message failed: {}", remote_peer, err);
            }

            common_apm::metrics::network::NETWORK_RECEIVED_MESSAGE_IN_PROCESSING_GUAGE.dec();
            common_apm::metrics::network::NETWORK_RECEIVED_IP_MESSAGE_IN_PROCESSING_GUAGE_VEC
                .with_label_values(&[&host])
                .dec();
        });
    }
}
