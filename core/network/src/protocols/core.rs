use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use futures::channel::mpsc::UnboundedSender;
use tentacle::{
    service::{ProtocolMeta, TargetProtocol},
    ProtocolId,
};
use tentacle_discovery::AddressManager;
use tentacle_identify::Callback;

use crate::{
    event::PeerManagerEvent,
    message::RawSessionMessage,
    protocols::{discovery::Discovery, identify::Identify, ping::Ping, transmitter::Transmitter},
    traits::NetworkProtocol,
};

pub const PING_PROTOCOL_ID: usize = 1;
pub const IDENTIFY_PROTOCOL_ID: usize = 2;
pub const DISCOVERY_PROTOCOL_ID: usize = 3;
pub const TRANSMITTERS_NUM: usize = 10;
pub const INIT_TRANSMITTER_PROTOCOL_ID: usize = 100;
pub const NEXT_TRANSMITTER_PROTOCOL_ID: AtomicUsize = AtomicUsize::new(100);

#[derive(Default)]
pub struct CoreProtocolBuilder<M, C> {
    ping:         Option<Ping>,
    identify:     Option<Identify<C>>,
    discovery:    Option<Discovery<M>>,
    transmitters: Vec<Transmitter>,
}

pub struct CoreProtocol {
    metas: Vec<ProtocolMeta>,
}

impl CoreProtocol {
    pub fn build<M, C>() -> CoreProtocolBuilder<M, C>
    where
        M: AddressManager + Send + 'static + Unpin,
        C: Callback + Send + 'static + Unpin,
    {
        CoreProtocolBuilder::new()
    }
}

impl NetworkProtocol for CoreProtocol {
    fn target() -> TargetProtocol {
        let mut tar_protos = vec![
            ProtocolId::new(PING_PROTOCOL_ID),
            ProtocolId::new(IDENTIFY_PROTOCOL_ID),
            ProtocolId::new(DISCOVERY_PROTOCOL_ID),
        ];

        let trans_protos = (0..TRANSMITTERS_NUM)
            .map(|i| ProtocolId::new(INIT_TRANSMITTER_PROTOCOL_ID + i))
            .collect::<Vec<_>>();

        tar_protos.extend(trans_protos);
        TargetProtocol::Multi(tar_protos)
    }

    fn metas(self) -> Vec<ProtocolMeta> {
        self.metas
    }

    fn transmitter_id() -> ProtocolId {
        let mut next_id = NEXT_TRANSMITTER_PROTOCOL_ID.fetch_add(1, Ordering::SeqCst);
        if next_id == INIT_TRANSMITTER_PROTOCOL_ID + TRANSMITTERS_NUM {
            next_id = INIT_TRANSMITTER_PROTOCOL_ID;
            NEXT_TRANSMITTER_PROTOCOL_ID.store(INIT_TRANSMITTER_PROTOCOL_ID + 1, Ordering::SeqCst);
        }

        ProtocolId::new(next_id)
    }
}

impl<M, C> CoreProtocolBuilder<M, C>
where
    M: AddressManager + Send + 'static + Unpin,
    C: Callback + Send + 'static + Unpin,
{
    pub fn new() -> Self {
        CoreProtocolBuilder {
            ping:         None,
            identify:     None,
            discovery:    None,
            transmitters: Vec::new(),
        }
    }

    pub fn ping(
        mut self,
        interval: Duration,
        timeout: Duration,
        event_tx: UnboundedSender<PeerManagerEvent>,
    ) -> Self {
        let ping = Ping::new(interval, timeout, event_tx);

        self.ping = Some(ping);
        self
    }

    pub fn identify(mut self, callback: C) -> Self {
        let identify = Identify::new(callback);

        self.identify = Some(identify);
        self
    }

    pub fn discovery(mut self, addr_mgr: M, sync_interval: Duration) -> Self {
        let discovery = Discovery::new(addr_mgr, sync_interval);

        self.discovery = Some(discovery);
        self
    }

    pub fn transmitters(mut self, bytes_tx: UnboundedSender<RawSessionMessage>) -> Self {
        let transmitters = (0..TRANSMITTERS_NUM)
            .map(|_| Transmitter::new(bytes_tx.clone()))
            .collect::<Vec<_>>();

        self.transmitters.extend(transmitters);
        self
    }

    pub fn build(self) -> CoreProtocol {
        let mut metas = Vec::new();

        let CoreProtocolBuilder {
            ping,
            identify,
            discovery,
            transmitters,
        } = self;

        // Panic early during protocol setup not runtime
        assert!(ping.is_some(), "init: missing protocol ping");
        assert!(identify.is_some(), "init: missing protocol identify");
        assert!(discovery.is_some(), "init: missing protocol discovery");
        assert!(
            !transmitters.is_empty(),
            "init: missing protocol transmitters"
        );

        if let Some(ping) = ping {
            metas.push(ping.build_meta(PING_PROTOCOL_ID.into()));
        }

        if let Some(identify) = identify {
            metas.push(identify.build_meta(IDENTIFY_PROTOCOL_ID.into()));
        }

        if let Some(discovery) = discovery {
            metas.push(discovery.build_meta(DISCOVERY_PROTOCOL_ID.into()));
        }

        for tran in transmitters {
            let protocol_id = NEXT_TRANSMITTER_PROTOCOL_ID.fetch_add(1, Ordering::SeqCst);
            metas.push(tran.build_meta(protocol_id.into()));
        }
        NEXT_TRANSMITTER_PROTOCOL_ID.store(INIT_TRANSMITTER_PROTOCOL_ID, Ordering::SeqCst);

        CoreProtocol { metas }
    }
}
