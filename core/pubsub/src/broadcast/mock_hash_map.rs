use std::any::Any;
use std::borrow::Borrow;
use std::cmp::Eq;
use std::collections::hash_map::ValuesMut;
use std::collections::HashMap as StdHashMap;
use std::fmt::Display;
use std::hash::Hash;

use futures::channel::mpsc;
use uuid::Uuid;

use crate::channel::broadcast;

pub type StateTx = mpsc::Sender<State>;
pub type StateRx = mpsc::Receiver<State>;

pub type State = (String, usize);

pub struct HashMap<K, V> {
    inner:        StdHashMap<K, V>,
    latest_entry: Option<K>,
    state_tx:     Option<StateTx>,
}

impl<K, V> HashMap<K, V>
where
    K: Eq + Hash + Send + 'static + Clone + Display,
    V: 'static,
{
    pub fn new() -> Self {
        HashMap {
            inner:        Default::default(),
            latest_entry: None,
            state_tx:     None,
        }
    }

    pub fn with_state_tx(state_tx: StateTx) -> Self {
        HashMap {
            inner:        Default::default(),
            latest_entry: None,
            state_tx:     Some(state_tx),
        }
    }

    pub fn insert(&mut self, key: K, val: V) {
        self.inner.insert(key.clone(), val);

        self.report_state(&key);
    }

    pub fn remove(&mut self, key: &K) {
        self.inner.remove(key);

        self.report_state(key);
    }

    pub fn entry(&mut self, key: K) -> &mut Self {
        self.latest_entry = Some(key);
        self
    }

    pub fn or_insert_with<F: FnOnce() -> V>(&mut self, default: F) -> &mut V {
        let mut val = default();

        assert!(
            self.latest_entry.is_some(),
            "call or_insert() before entry()"
        );
        let entry = self.latest_entry.take().unwrap();

        let any_val: &mut dyn Any = &mut val;
        if let Some(hashmap) = any_val.downcast_mut::<HashMap<Uuid, broadcast::Sender>>() {
            hashmap.state_tx = self.state_tx.clone();
        }

        if !self.inner.contains_key(&entry) {
            self.inner.insert(entry.clone(), val);
            self.report_state(&entry)
        }
        self.latest_entry = None;

        assert!(
            self.inner.contains_key(&entry),
            "fail to insert entry: {}",
            entry
        );
        self.get_mut(&entry).unwrap()
    }

    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        self.inner.values_mut()
    }

    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.inner.get_mut(key)
    }

    pub fn borrow_inner(&self) -> &StdHashMap<K, V> {
        <Self as Borrow<StdHashMap<K, V>>>::borrow(self)
    }

    fn report_state(&mut self, key: &K) {
        if let Some(tx) = &mut self.state_tx {
            let state = (key.to_string(), self.inner.len());

            tx.try_send(state).unwrap();
        }
    }
}

impl<K, V> Borrow<StdHashMap<K, V>> for HashMap<K, V> {
    fn borrow(&self) -> &StdHashMap<K, V> {
        &self.inner
    }
}
