use jsonrpc_types::rpctypes::Filter;
use transient_hashmap::{StandardTimer, Timer, TransientHashMap};

const FILTER_LIFETIME: u32 = 60;
pub type BlockNumber = u64;
pub type FilterId = usize;

#[derive(Clone)]
pub enum FilterType {
    /// block filter
    /// ref: <https://docs.citahub.com/en-US/cita/rpc-guide/rpc#newblockfilter>
    Block(BlockNumber),
    /// logs filter
    /// ref: <https://docs.citahub.com/en-US/cita/rpc-guide/rpc#newfilter>
    Logs(BlockNumber, Filter),
}

pub struct FilterManager<F, T = StandardTimer>
where
    T: Timer,
{
    filters: TransientHashMap<FilterId, F, T>,
    next_available_id: FilterId,
}

impl<F> FilterManager<F, StandardTimer> {
    pub fn default() -> Self {
        FilterManager::new_with_timer(Default::default())
    }
}

impl<F, T> FilterManager<F, T>
where
    T: Timer,
{
    pub fn new_with_timer(timer: T) -> Self {
        FilterManager {
            filters: TransientHashMap::new_with_timer(FILTER_LIFETIME, timer),
            next_available_id: 0,
        }
    }

    pub fn new_filter(&mut self, filter: F) -> FilterId {
        self.filters.prune();

        let id = self.next_available_id;
        self.filters.insert(id, filter);

        self.next_available_id += 1;
        id
    }

    pub fn get(&mut self, id: FilterId) -> Option<&F> {
        self.filters.prune();
        self.filters.get(&id)
    }

    pub fn get_mut(&mut self, id: FilterId) -> Option<&mut F> {
        self.filters.prune();
        self.filters.get_mut(&id)
    }

    pub fn uninstall_filter(&mut self, id: FilterId) {
        self.filters.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use transient_hashmap::Timer;

    struct TestTimer<'a> {
        time: &'a Cell<i64>,
    }

    impl<'a> Timer for TestTimer<'a> {
        fn get_time(&self) -> i64 {
            self.time.get()
        }
    }

    #[test]
    fn test_filter_manager() {
        let time = Cell::new(0);
        let timer = TestTimer { time: &time };

        let mut fm = FilterManager::new_with_timer(timer);
        assert_eq!(fm.new_filter(20), 0);
        assert_eq!(fm.new_filter(20), 1);

        time.set(10);
        *fm.get_mut(0).unwrap() = 21;
        assert_eq!(*fm.get(0).unwrap(), 21);
        assert_eq!(*fm.get(1).unwrap(), 20);

        time.set(30);
        *fm.get_mut(1).unwrap() = 23;
        assert_eq!(*fm.get(1).unwrap(), 23);

        time.set(75);
        assert!(fm.get(0).is_none());
        assert_eq!(*fm.get(1).unwrap(), 23);

        fm.uninstall_filter(1);
        assert!(fm.get(1).is_none());
    }
}
