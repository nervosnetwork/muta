use async_trait::async_trait;
use protocol::{
    traits::{Context, Gossip, MessageCodec, Priority},
    Bytes, ProtocolResult,
};
use tentacle::{secio::PeerId, service::TargetSession};

use crate::{
    endpoint::Endpoint,
    error::NetworkError,
    message::{Headers, NetworkMessage},
    traits::{Compression, MessageSender},
    PeerIdExt,
};

#[derive(Clone)]
pub struct NetworkGossip<S, C> {
    sender:      S,
    compression: C,
}

impl<S, C> NetworkGossip<S, C>
where
    S: MessageSender + Sync + Send + Clone,
    C: Compression + Sync + Send + Clone,
{
    pub fn new(sender: S, compression: C) -> Self {
        NetworkGossip {
            sender,
            compression,
        }
    }

    async fn package_message<M>(&self, ctx: Context, end: &str, mut msg: M) -> ProtocolResult<Bytes>
    where
        M: MessageCodec,
    {
        let endpoint = end.parse::<Endpoint>()?;
        let data = msg.encode().await?;
        let mut headers = Headers::default();
        if let Some(state) = common_apm::muta_apm::MutaTracer::span_state(&ctx) {
            headers.set_trace_id(state.trace_id());
            headers.set_span_id(state.span_id());
            log::info!("no trace id found for gossip {}", endpoint.full_url());
        }
        let net_msg = NetworkMessage::new(endpoint, data, headers)
            .encode()
            .await?;
        let msg = self.compression.compress(net_msg)?;

        Ok(msg)
    }

    fn send(
        &self,
        _ctx: Context,
        tar: TargetSession,
        msg: Bytes,
        pri: Priority,
    ) -> Result<(), NetworkError> {
        self.sender.send(tar, msg, pri)
    }

    async fn multisend<'a, P: AsRef<[Bytes]> + 'a>(
        &self,
        _ctx: Context,
        peer_ids: P,
        msg: Bytes,
        pri: Priority,
    ) -> Result<(), NetworkError> {
        let peer_ids = {
            let byteses = peer_ids.as_ref().iter();
            let maybe_ids = byteses.map(<PeerId as PeerIdExt>::from_bytes);

            maybe_ids.collect::<Result<Vec<_>, _>>()?
        };

        self.sender.multisend(peer_ids, msg, pri).await
    }
}

#[async_trait]
impl<S, C> Gossip for NetworkGossip<S, C>
where
    S: MessageSender + Sync + Send + Clone,
    C: Compression + Sync + Send + Clone,
{
    async fn broadcast<M>(&self, cx: Context, end: &str, msg: M, p: Priority) -> ProtocolResult<()>
    where
        M: MessageCodec,
    {
        let msg = self.package_message(cx.clone(), end, msg).await?;
        self.send(cx, TargetSession::All, msg, p)?;
        common_apm::metrics::network::on_network_message_sent_all_target(end);
        Ok(())
    }

    async fn multicast<'a, M, P>(
        &self,
        cx: Context,
        end: &str,
        peer_ids: P,
        msg: M,
        p: Priority,
    ) -> ProtocolResult<()>
    where
        M: MessageCodec,
        P: AsRef<[Bytes]> + Send + 'a,
    {
        let msg = self.package_message(cx.clone(), end, msg).await?;
        let multicast_count = peer_ids.as_ref().len();

        self.multisend(cx, peer_ids, msg, p).await?;

        common_apm::metrics::network::on_network_message_sent_multi_target(
            end,
            multicast_count as i64,
        );
        Ok(())
    }
}
