use async_trait::async_trait;
use protocol::traits::{Context, Gossip, MessageCodec, Priority};
use protocol::{Bytes, ProtocolResult};
use tentacle::secio::PeerId;
use tentacle::service::TargetSession;

use crate::endpoint::Endpoint;
use crate::error::NetworkError;
use crate::message::{Headers, NetworkMessage};
use crate::protocols::{Recipient, Transmitter, TransmitterMessage};
use crate::traits::{Compression, NetworkContext};
use crate::PeerIdExt;

#[derive(Clone)]
pub struct NetworkGossip {
    transmitter: Transmitter,
}

impl NetworkGossip {
    pub fn new(transmitter: Transmitter) -> Self {
        NetworkGossip { transmitter }
    }

    async fn package_message<M>(
        &self,
        ctx: Context,
        endpoint: &str,
        mut msg: M,
    ) -> ProtocolResult<Bytes>
    where
        M: MessageCodec,
    {
        let endpoint = endpoint.parse::<Endpoint>()?;
        let data = msg.encode()?;
        let mut headers = Headers::default();
        if let Some(state) = common_apm::muta_apm::MutaTracer::span_state(&ctx) {
            headers.set_trace_id(state.trace_id());
            headers.set_span_id(state.span_id());
            log::info!("no trace id found for gossip {}", endpoint.full_url());
        }
        let net_msg = NetworkMessage::new(endpoint, data, headers).encode()?;
        let msg = self.transmitter.compressor().compress(net_msg)?;

        Ok(msg)
    }

    async fn send_to_sessions(
        &self,
        ctx: Context,
        target_session: TargetSession,
        data: Bytes,
        priority: Priority,
    ) -> Result<(), NetworkError> {
        let msg = TransmitterMessage {
            recipient: Recipient::Session(target_session),
            priority,
            data,
            ctx,
        };

        self.transmitter.behaviour.send(msg).await
    }

    async fn send_to_peers<'a, P: AsRef<[Bytes]> + 'a>(
        &self,
        ctx: Context,
        peer_ids: P,
        data: Bytes,
        priority: Priority,
    ) -> Result<(), NetworkError> {
        let peer_ids = {
            let byteses = peer_ids.as_ref().iter();
            let maybe_ids = byteses.map(<PeerId as PeerIdExt>::from_bytes);

            maybe_ids.collect::<Result<Vec<_>, _>>()?
        };

        let msg = TransmitterMessage {
            recipient: Recipient::PeerId(peer_ids),
            priority,
            data,
            ctx,
        };

        self.transmitter.behaviour.send(msg).await
    }
}

#[async_trait]
impl Gossip for NetworkGossip {
    async fn broadcast<M>(
        &self,
        mut cx: Context,
        endpoint: &str,
        msg: M,
        priority: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        let msg = self.package_message(cx.clone(), endpoint, msg).await?;
        let ctx = cx.set_url(endpoint.to_owned());
        self.send_to_sessions(ctx, TargetSession::All, msg, priority)
            .await?;
        common_apm::metrics::network::on_network_message_sent_all_target(endpoint);
        Ok(())
    }

    async fn multicast<'a, M, P>(
        &self,
        mut cx: Context,
        endpoint: &str,
        peer_ids: P,
        msg: M,
        priority: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec,
        P: AsRef<[Bytes]> + Send + 'a,
    {
        let msg = self.package_message(cx.clone(), endpoint, msg).await?;
        let multicast_count = peer_ids.as_ref().len();

        let ctx = cx.set_url(endpoint.to_owned());
        self.send_to_peers(ctx, peer_ids, msg, priority).await?;
        common_apm::metrics::network::on_network_message_sent_multi_target(
            endpoint,
            multicast_count as i64,
        );
        Ok(())
    }
}
