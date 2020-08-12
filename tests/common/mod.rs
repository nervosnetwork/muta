#![allow(clippy::mutable_key_type)]

pub mod node;

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};

use protocol::types::Hash;
use protocol::BytesMut;
use rand::{rngs::OsRng, RngCore};

static AVAILABLE_PORT: AtomicU16 = AtomicU16::new(2000);

pub fn tmp_dir() -> PathBuf {
    let mut tmp_dir = std::env::temp_dir();
    let sub_dir = {
        let mut random_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut random_bytes);
        Hash::digest(BytesMut::from(random_bytes.as_ref()).freeze()).as_hex()
    };

    tmp_dir.push(sub_dir + "/");
    tmp_dir
}

pub fn available_port_pair() -> (u16, u16) {
    (available_port(), available_port())
}

fn available_port() -> u16 {
    let is_available = |port| -> bool { TcpListener::bind(("127.0.0.1", port)).is_ok() };

    loop {
        let port = AVAILABLE_PORT.fetch_add(1, Ordering::SeqCst);
        if is_available(port) {
            return port;
        }
    }
}
