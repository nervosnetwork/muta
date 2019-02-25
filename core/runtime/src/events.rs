use core_types::{Block, SignedTransaction};

pub enum EventType {
    NewTransaction(SignedTransaction),
    NewHeight(Block),
    Synchronized(Block),
}

// TODO: define event trait
