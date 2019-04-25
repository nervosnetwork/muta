// FIXME: strange clippy bug, it report on line: 189
#![allow(clippy::needless_lifetimes)]

#[cfg(test)]
mod mock_hash_map;

use std::collections::VecDeque;
use std::sync::Arc;

use futures::channel::{mpsc, oneshot};
use futures::prelude::{FutureExt, StreamExt};
use futures::select;
use futures::stream::select_all;
#[cfg(not(test))]
use hashbrown::HashMap;
use log::warn;
use uuid::Uuid;

#[cfg(test)]
use crate::broadcast::mock_hash_map::HashMap;
use crate::channel::broadcast::{self as broadcast, Event, Message};

#[derive(Debug)]
pub enum Action {
    NewPub {
        topic: String,
        rx:    mpsc::Receiver<Event>,
    },

    RemovePub {
        topic: String,
    },

    NewSub {
        topic: String,
        uuid:  Uuid,
        tx:    mpsc::Sender<Message>,
    },

    RemoveSub {
        topic: String,
        uuid:  Uuid,
    },
}

pub type Publishers = HashMap<String, broadcast::Receiver>;
pub type Subscribers = HashMap<String, HashMap<Uuid, broadcast::Sender>>;
pub type ActionReceiver = mpsc::Receiver<Action>;
pub type PendingActions = VecDeque<Action>;

pub struct Broadcast;

impl Broadcast {
    pub async fn broadcast(
        mut pubs: Publishers,
        mut subs: Subscribers,
        mut pending_acts: PendingActions,
        act_rx: ActionReceiver,
        shutdown_rx: oneshot::Receiver<()>,
    ) {
        let pubs = &mut pubs;
        let subs = &mut subs;
        let pending_acts = &mut pending_acts;

        let mut act_rx = act_rx.fuse();
        let mut shutdown_rx = shutdown_rx.fuse();

        loop {
            Self::handle_actions(pending_acts, pubs, subs);
            let mut select_pubs = select_all(pubs.values_mut());

            select! {
                _ = shutdown_rx => break,
                action = act_rx.next() => Self::save_action(pending_acts, action),
                event = select_pubs.next() => Self::do_broadcast(event, subs),
                complete => break,
            }
        }
    }

    #[inline]
    fn save_action(pending_acts: &mut VecDeque<Action>, action: Option<Action>) {
        if let Some(action) = action {
            pending_acts.push_back(action);
        }
    }

    #[inline]
    fn handle_actions(
        pending_acts: &mut VecDeque<Action>,
        pubs: &mut Publishers,
        subs: &mut Subscribers,
    ) {
        for action in pending_acts.drain(..) {
            match action {
                Action::NewPub { topic, rx } => {
                    let rx = broadcast::Receiver::new(rx);
                    pubs.insert(topic, rx);
                }
                Action::RemovePub { topic } => {
                    pubs.remove(&topic);
                    subs.remove(&topic);
                }
                Action::NewSub { topic, uuid, tx } => {
                    let tx = broadcast::Sender::new(uuid, tx);
                    let subs = subs.entry(topic).or_insert(HashMap::new());

                    subs.insert(uuid, tx);
                }
                Action::RemoveSub { topic, uuid } => {
                    let subs = subs.entry(topic).or_insert(HashMap::new());
                    subs.remove(&uuid);
                }
            }
        }
    }

    #[inline]
    fn do_broadcast(opt_event: Option<Event>, subs: &mut Subscribers) {
        if let Some(event) = opt_event {
            let topic = event.topic();
            let msg = event.message();

            if let Some(subs) = subs.get_mut(topic) {
                for sub in subs.values_mut() {
                    if let Err(e) = sub.try_send(Arc::clone(&msg)) {
                        warn!("{}: send failure on {}: {:?}", topic, sub.uuid(), e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread::{spawn, JoinHandle};

    use futures::channel::{mpsc, oneshot};
    use futures::executor::block_on;
    use futures::prelude::StreamExt;
    use uuid::Uuid;

    use crate::broadcast::mock_hash_map::{State, StateRx};
    use crate::channel::broadcast::{BroadcastEvent, Event, Message};

    use super::{Action, Broadcast, PendingActions, Publishers, Subscribers};

    #[derive(Debug)]
    struct TestEvent {
        topic: String,
        msg:   Message,
    }

    impl TestEvent {
        pub fn new(topic: String, msg: String) -> Self {
            let msg: Message = Arc::new(Box::new(msg));

            TestEvent { topic, msg }
        }
    }

    impl BroadcastEvent for TestEvent {
        fn topic(&self) -> &str {
            &self.topic
        }

        fn message(&self) -> &Message {
            &self.msg
        }

        fn boxed(self) -> Event {
            Box::new(self)
        }
    }

    struct Control {
        act_tx:      mpsc::Sender<Action>,
        shutdown_tx: oneshot::Sender<()>,
        state_rx:    StateRx,

        handle: JoinHandle<()>,
    }

    impl Control {
        pub fn shutdown(self) {
            self.shutdown_tx.send(()).unwrap();
            self.handle.join().unwrap();
        }

        pub async fn get_next_state(&mut self) -> State {
            await!(self.state_rx.next()).unwrap()
        }

        pub fn new_pub_sub(
            &mut self,
            topic: String,
        ) -> (mpsc::Sender<Event>, mpsc::Receiver<Message>, Uuid) {
            let (pub_tx, pub_rx) = mpsc::channel(10);
            let (sub_tx, sub_rx) = mpsc::channel(10);
            let uuid = Uuid::new_v4();

            let new_pub = Action::NewPub {
                topic: topic.clone(),
                rx:    pub_rx,
            };
            let new_sub = Action::NewSub {
                topic: topic.clone(),
                uuid,
                tx: sub_tx,
            };

            self.act_tx.try_send(new_pub).unwrap();
            self.act_tx.try_send(new_sub).unwrap();

            // pub inserted
            let (topic, len) = block_on(self.get_next_state());
            assert_eq!(topic, "test");
            assert_eq!(len, 1);

            // sub inserted
            let (_, len) = block_on(self.get_next_state());
            assert_eq!(len, 1);

            let (sub_uuid, len) = block_on(self.get_next_state());
            assert_eq!(sub_uuid, uuid.to_string());
            assert_eq!(len, 1);

            (pub_tx, sub_rx, uuid)
        }
    }

    fn new_broadcast() -> Control {
        let pending_acts = PendingActions::new();
        let (state_tx, state_rx) = mpsc::channel(20);
        let pubs = Publishers::with_state_tx(state_tx.clone());
        let subs = Subscribers::with_state_tx(state_tx.clone());

        let (act_tx, act_rx) = mpsc::channel(20);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let broadcast = Broadcast::broadcast(pubs, subs, pending_acts, act_rx, shutdown_rx);
        let handle = spawn(move || block_on(broadcast));

        Control {
            act_tx,
            shutdown_tx,
            state_rx,

            handle,
        }
    }

    #[test]
    fn test_new_pub() {
        let mut ctrl = new_broadcast();

        let topic = "test".to_owned();
        let (_tx, rx) = mpsc::channel(10);
        let new_pub = Action::NewPub { topic, rx };

        ctrl.act_tx.try_send(new_pub).unwrap();

        let (topic, len) = block_on(ctrl.get_next_state());
        assert_eq!(topic, "test");
        assert_eq!(len, 1);

        ctrl.shutdown();
    }

    #[test]
    fn test_remove_pub() {
        let mut ctrl = new_broadcast();

        let topic = "test";
        ctrl.new_pub_sub(topic.to_owned());

        let rm_pub = Action::RemovePub {
            topic: topic.to_owned(),
        };

        ctrl.act_tx.try_send(rm_pub).unwrap();

        // remove pub, should also remove sub
        let (_, len) = block_on(ctrl.get_next_state());
        assert_eq!(len, 0);

        let (_, len) = block_on(ctrl.get_next_state());
        assert_eq!(len, 0);

        ctrl.shutdown();
    }

    #[test]
    fn test_new_sub() {
        let mut ctrl = new_broadcast();

        let topic = "test".to_owned();
        let (tx, _rx) = mpsc::channel(10);
        let uuid = Uuid::new_v4();
        let new_sub = Action::NewSub { topic, uuid, tx };

        ctrl.act_tx.try_send(new_sub).unwrap();

        let (_, len) = block_on(ctrl.get_next_state());
        assert_eq!(len, 1);

        let (sub_uuid, len) = block_on(ctrl.get_next_state());
        assert_eq!(sub_uuid, uuid.to_string());
        assert_eq!(len, 1);

        ctrl.shutdown();
    }

    #[test]
    fn test_remove_sub() {
        let mut ctrl = new_broadcast();
        let topic = "test";

        let (_, _, sub_uuid) = ctrl.new_pub_sub(topic.to_owned());

        let rm_sub = Action::RemoveSub {
            topic: topic.to_owned(),
            uuid:  sub_uuid,
        };

        ctrl.act_tx.try_send(rm_sub).unwrap();

        let (_, len) = block_on(ctrl.get_next_state());
        assert_eq!(len, 0);

        ctrl.shutdown();
    }

    #[test]
    fn test_broadcast() {
        let mut ctrl = new_broadcast();
        let topic = "test";
        let msg = "coaerl & tsuaedy".to_owned();

        let (mut pub_tx, mut sub_rx, _) = ctrl.new_pub_sub(topic.to_owned());

        let test_event = TestEvent::new(topic.to_owned(), msg.to_owned());
        pub_tx.try_send(Box::new(test_event)).unwrap();

        let any_msg = block_on(sub_rx.next()).unwrap();
        let recv_msg = any_msg.downcast_ref::<String>().unwrap().to_owned();

        assert_eq!(recv_msg, msg);

        ctrl.shutdown();
    }

    #[test]
    fn test_broadcast_use_wrong_recv_msg_type() {
        let mut ctrl = new_broadcast();
        let topic = "test";
        let msg = "coalre & tdusaey".to_owned();

        let (mut pub_tx, mut sub_rx, _) = ctrl.new_pub_sub(topic.to_owned());

        let test_event = TestEvent::new(topic.to_owned(), msg.to_owned());
        pub_tx.try_send(Box::new(test_event)).unwrap();

        let any_msg = block_on(sub_rx.next()).unwrap();
        let msg = any_msg.downcast_ref::<usize>(); // should be String

        assert!(msg.is_none());

        ctrl.shutdown();
    }

    #[test]
    fn test_broadcast_shutdown() {
        let ctrl = new_broadcast();
        let act_tx = ctrl.act_tx.clone();

        ctrl.shutdown();

        assert!(act_tx.is_closed());
    }

    #[test]
    fn test_handle_actions_new_pub() {
        let mut pending_acts = PendingActions::new();
        let mut pubs = Publishers::new();
        let mut subs = Subscribers::new();

        let topic = "shanbala";
        let (_, rx) = mpsc::channel(10);
        let edo = Action::NewPub {
            topic: topic.to_owned(),
            rx,
        };

        let topic = "east_state";
        let (_, rx) = mpsc::channel(10);
        let alfans = Action::NewPub {
            topic: topic.to_owned(),
            rx,
        };

        pending_acts.push_back(edo);
        pending_acts.push_back(alfans);

        Broadcast::handle_actions(&mut pending_acts, &mut pubs, &mut subs);

        let state = pubs.borrow_inner();
        assert_eq!(state.len(), 2);
        assert!(state.contains_key("shanbala"));
        assert!(state.contains_key("east_state"));
    }

    #[test]
    fn test_handle_actions_remove_pub() {
        let mut pending_acts = PendingActions::new();
        let mut pubs = Publishers::new();
        let mut subs = Subscribers::new();

        let topic = "siki";
        let (_, rx) = mpsc::channel(10);
        let black_glass = Action::NewPub {
            topic: topic.to_owned(),
            rx,
        };

        let uuid = Uuid::new_v4();
        let (tx, _) = mpsc::channel(10);
        let sora_orange = Action::NewSub {
            topic: topic.to_owned(),
            uuid,
            tx,
        };

        pending_acts.push_back(black_glass);
        pending_acts.push_back(sora_orange);

        Broadcast::handle_actions(&mut pending_acts, &mut pubs, &mut subs);

        {
            let pubs_state = pubs.borrow_inner();
            let subs_state = subs.borrow_inner();
            assert_eq!(pubs_state.len(), 1);
            assert_eq!(subs_state.len(), 1);
            assert!(pubs_state.contains_key(topic));
            assert!(subs_state.contains_key(topic));

            let subs_sub_state = subs_state.get(topic).unwrap().borrow_inner();
            assert!(subs_sub_state.contains_key(&uuid));
            assert_eq!(subs_sub_state.len(), 1);
        }

        let glass_sleep = Action::RemovePub {
            topic: topic.to_owned(),
        };

        pending_acts.push_back(glass_sleep);
        Broadcast::handle_actions(&mut pending_acts, &mut pubs, &mut subs);

        let pubs_state = pubs.borrow_inner();
        let subs_state = subs.borrow_inner();

        assert_eq!(pubs_state.len(), 0);
        assert_eq!(subs_state.len(), 0);
    }

    #[test]
    fn test_handle_actions_new_sub() {
        let mut pending_acts = PendingActions::new();
        let mut pubs = Publishers::new();
        let mut subs = Subscribers::new();

        let topic = "kaily";

        let uuid = Uuid::new_v4();
        let (tx, _) = mpsc::channel(10);
        let sora = Action::NewSub {
            topic: topic.to_owned(),
            uuid,
            tx,
        };

        pending_acts.push_back(sora);

        Broadcast::handle_actions(&mut pending_acts, &mut pubs, &mut subs);

        let subs_state = subs.borrow_inner();
        assert!(subs_state.contains_key(topic));
        assert_eq!(subs_state.len(), 1);

        let subs_sub_state = subs_state.get(topic).unwrap().borrow_inner();
        assert!(subs_sub_state.contains_key(&uuid));
        assert_eq!(subs_sub_state.len(), 1);
    }

    #[test]
    fn test_handle_actions_remove_sub() {
        let mut pending_acts = PendingActions::new();
        let mut pubs = Publishers::new();
        let mut subs = Subscribers::new();

        let topic = "kaily";

        let uuid = Uuid::new_v4();
        let (tx, _) = mpsc::channel(10);
        let sora = Action::NewSub {
            topic: topic.to_owned(),
            uuid,
            tx,
        };

        pending_acts.push_back(sora);

        Broadcast::handle_actions(&mut pending_acts, &mut pubs, &mut subs);

        {
            let subs_state = subs.borrow_inner();
            assert!(subs_state.contains_key(topic));
            assert_eq!(subs_state.len(), 1);

            let subs_sub_state = subs_state.get(topic).unwrap().borrow_inner();
            assert!(subs_sub_state.contains_key(&uuid));
            assert_eq!(subs_sub_state.len(), 1);
        }

        let sora_drop = Action::RemoveSub {
            topic: topic.to_owned(),
            uuid,
        };

        pending_acts.push_back(sora_drop);
        Broadcast::handle_actions(&mut pending_acts, &mut pubs, &mut subs);

        let subs_state = subs.borrow_inner();
        assert_eq!(subs_state.len(), 1);

        let subs_sub_state = subs_state.get(topic).unwrap().borrow_inner();
        assert_eq!(subs_sub_state.len(), 0);
    }
}
