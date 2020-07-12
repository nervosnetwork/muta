use super::{
    addr::{AddressManager, ConnectableAddr, DEFAULT_MAX_KNOWN},
    message::{DiscoveryMessage, Node, Nodes},
    substream::{RemoteAddress, Substream, SubstreamKey, SubstreamValue},
};

use futures::{
    channel::mpsc::{channel, Receiver, Sender},
    stream::FusedStream,
    Stream,
};
use log::debug;
use rand::seq::SliceRandom;
use tentacle::{
    multiaddr::Multiaddr,
    utils::{is_reachable, multiaddr_to_socketaddr},
    SessionId,
};
use tokio::time::Interval;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

const CHECK_INTERVAL: Duration = Duration::from_secs(3);

pub struct DiscoveryBehaviour {
    // Default: 5000
    max_known: usize,

    // Address Manager
    addr_mgr: AddressManager,

    // The Nodes not yet been yield
    pending_nodes: VecDeque<(SubstreamKey, SessionId, Nodes)>,

    // For manage those substreams
    substreams: HashMap<SubstreamKey, SubstreamValue>,

    // For add new substream to Discovery
    substream_sender:   Sender<Substream>,
    // For add new substream to Discovery
    substream_receiver: Receiver<Substream>,

    dead_keys: HashSet<SubstreamKey>,

    dynamic_query_cycle: Option<Duration>,

    check_interval: Option<Interval>,
}

#[derive(Clone)]
pub struct DiscoveryBehaviourHandle {
    pub substream_sender: Sender<Substream>,
}

impl DiscoveryBehaviour {
    /// Query cycle means checking and synchronizing the cycle time of the
    /// currently connected node, default is 24 hours
    pub fn new(addr_mgr: AddressManager, query_cycle: Option<Duration>) -> DiscoveryBehaviour {
        let (substream_sender, substream_receiver) = channel(8);
        DiscoveryBehaviour {
            check_interval: None,
            max_known: DEFAULT_MAX_KNOWN,
            addr_mgr,
            pending_nodes: VecDeque::default(),
            substreams: HashMap::default(),
            substream_sender,
            substream_receiver,
            dead_keys: HashSet::default(),
            dynamic_query_cycle: query_cycle,
        }
    }

    pub fn handle(&self) -> DiscoveryBehaviourHandle {
        DiscoveryBehaviourHandle {
            substream_sender: self.substream_sender.clone(),
        }
    }

    fn recv_substreams(&mut self, cx: &mut Context) {
        loop {
            if self.substream_receiver.is_terminated() {
                break;
            }

            match Pin::new(&mut self.substream_receiver)
                .as_mut()
                .poll_next(cx)
            {
                Poll::Ready(Some(substream)) => {
                    let key = substream.key();
                    debug!("Received a substream: key={:?}", key);
                    let value = SubstreamValue::new(
                        key.direction,
                        substream,
                        self.max_known,
                        self.dynamic_query_cycle,
                    );
                    self.substreams.insert(key, value);
                }
                Poll::Ready(None) => unreachable!(),
                Poll::Pending => {
                    debug!("DiscoveryBehaviour.substream_receiver Async::NotReady");
                    break;
                }
            }
        }
    }

    fn check_interval(&mut self, cx: &mut Context) {
        if self.check_interval.is_none() {
            self.check_interval = Some(tokio::time::interval(CHECK_INTERVAL));
        }
        let mut interval = self.check_interval.take().unwrap();
        loop {
            match Pin::new(&mut interval).as_mut().poll_next(cx) {
                Poll::Ready(Some(_)) => {}
                Poll::Ready(None) => {
                    debug!("DiscoveryBehaviour check_interval poll finished");
                    break;
                }
                Poll::Pending => break,
            }
        }
        self.check_interval = Some(interval);
    }

    fn poll_substreams(&mut self, cx: &mut Context, announce_multiaddrs: &mut Vec<Multiaddr>) {
        #[cfg(feature = "global_ip_only")]
        let global_ip_only = true;
        #[cfg(not(feature = "global_ip_only"))]
        let global_ip_only = false;

        let announce_fn = |announce_multiaddrs: &mut Vec<Multiaddr>, addr: &Multiaddr| {
            if !global_ip_only
                || multiaddr_to_socketaddr(addr)
                    .map(|addr| is_reachable(addr.ip()))
                    .unwrap_or_default()
            {
                announce_multiaddrs.push(addr.clone());
            }
        };
        for (key, value) in self.substreams.iter_mut() {
            value.check_timer();

            match value.receive_messages(cx, &mut self.addr_mgr) {
                Ok(Some((session_id, nodes_list))) => {
                    for nodes in nodes_list {
                        self.pending_nodes
                            .push_back((key.clone(), session_id, nodes));
                    }
                }
                Ok(None) => {
                    // stream close
                    self.dead_keys.insert(key.clone());
                }
                Err(err) => {
                    debug!("substream {:?} receive messages error: {:?}", key, err);
                    // remove the substream
                    self.dead_keys.insert(key.clone());
                }
            }

            match value.send_messages(cx) {
                Ok(_) => {}
                Err(err) => {
                    debug!("substream {:?} send messages error: {:?}", key, err);
                    // remove the substream
                    self.dead_keys.insert(key.clone());
                }
            }

            if value.announce {
                if let RemoteAddress::Listen(ref addr) = value.remote_addr {
                    announce_fn(announce_multiaddrs, addr)
                }
                value.announce = false;
                value.last_announce = Some(Instant::now());
            }
        }
    }

    fn remove_dead_stream(&mut self) {
        let mut dead_addr = Vec::default();
        for key in self.dead_keys.drain() {
            if let Some(addr) = self.substreams.remove(&key) {
                dead_addr.push(ConnectableAddr::from(addr.remote_addr.into_inner()));
            }
        }

        if !dead_addr.is_empty() {
            self.substreams
                .values_mut()
                .for_each(|value| value.addr_known.remove(dead_addr.iter()));
        }
    }

    fn send_messages(&mut self, cx: &mut Context) {
        for (key, value) in self.substreams.iter_mut() {
            let announce_multiaddrs = value.announce_multiaddrs.split_off(0);
            if !announce_multiaddrs.is_empty() {
                let items = announce_multiaddrs
                    .into_iter()
                    .map(|addr| Node {
                        addresses: vec![addr],
                    })
                    .collect::<Vec<_>>();
                let nodes = Nodes {
                    announce: true,
                    items,
                };
                value
                    .pending_messages
                    .push_back(DiscoveryMessage::Nodes(nodes));
            }

            match value.send_messages(cx) {
                Ok(_) => {}
                Err(err) => {
                    debug!("substream {:?} send messages error: {:?}", key, err);
                    // remove the substream
                    self.dead_keys.insert(key.clone());
                }
            }
        }
    }
}

impl Stream for DiscoveryBehaviour {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        debug!("DiscoveryBehaviour.poll()");
        self.recv_substreams(cx);
        self.check_interval(cx);

        let mut announce_multiaddrs = Vec::new();

        self.poll_substreams(cx, &mut announce_multiaddrs);

        self.remove_dead_stream();

        let mut rng = rand::thread_rng();
        let mut remain_keys = self.substreams.keys().cloned().collect::<Vec<_>>();
        debug!("announce_multiaddrs: {:?}", announce_multiaddrs);
        for announce_multiaddr in announce_multiaddrs.into_iter() {
            let announce_addr = ConnectableAddr::from(announce_multiaddr.clone());
            remain_keys.shuffle(&mut rng);
            for i in 0..2 {
                if let Some(key) = remain_keys.get(i) {
                    if let Some(value) = self.substreams.get_mut(key) {
                        debug!(
                            ">> send {} to: {:?}, contains: {}",
                            announce_multiaddr,
                            value.remote_addr,
                            value.addr_known.contains(&announce_addr)
                        );
                        if value.announce_multiaddrs.len() < 10
                            && !value.addr_known.contains(&announce_addr)
                        {
                            value.announce_multiaddrs.push(announce_multiaddr.clone());
                            value.addr_known.insert(announce_addr.clone());
                        }
                    }
                }
            }
        }

        self.send_messages(cx);

        match self.pending_nodes.pop_front() {
            Some((_key, session_id, nodes)) => {
                let addrs = nodes
                    .items
                    .into_iter()
                    .flat_map(|node| node.addresses.into_iter())
                    .collect::<Vec<_>>();
                self.addr_mgr.add_new_addrs(session_id, addrs);
                Poll::Ready(Some(()))
            }
            None => Poll::Pending,
        }
    }
}
