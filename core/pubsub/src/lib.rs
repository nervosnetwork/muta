//! ## Summary
//!
//! This crate provides a simple pub/usb.
//!
//! **_ This crate requires Rust nightly. _**
//!
//! ### Basic Idea
//!
//! #### Publish
//!
//! ```text
//! Pub => Generic Message => pubsub::Sender => Any => broadcast::Receiver
//! ```
//!
//! #### Subscribe
//!
//! ```text
//! broadcast::Receiver => broadcast::Sender => pubsub::Receiver => Generic Message => Sub
//! ```
//!
//! ### Usage
//!
//! To use this crate, you need to start with a Rust 2018 edition crate, with
//! rustc 1.33.0-nightly or later.
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! # In the `[packages]` section
//! edition = "2018"
//!
//! # In the `[dependencies]` section
//! core_pubsub = { path = "{path to this crate}" }
//! ```
//!
//! Then, get started. In your application, add:
//!
//! ```rust
//! // The nightly features that are commonly needed with async/await
//! #![feature(await_macro, async_await, futures_api)]
//!
//! use std::thread::spawn;
//!
//! use futures::executor::block_on;
//! use futures::future::ready;
//! use futures::prelude::StreamExt;
//!
//! use core_pubsub::PubSub;
//!
//! #[derive(Clone, Debug)]
//! struct Message {
//!     header: String,
//!     body:   String,
//! }
//!
//! pub fn main() -> Result<(), ()> {
//!     let mut pubsub = PubSub::builder().build().start();
//!
//!     let mut sub = pubsub.subscribe::<Message>("test".to_owned())?;
//!
//!     let sub_two = pubsub.subscribe::<Message>("test".to_owned())?;
//!     pubsub.unsubscribe("test".to_owned(), sub_two.uuid())?;
//!
//!     let mut register = pubsub.register();
//!
//!     let mut pubb = register.publish::<Message>("test".to_owned())?;
//!     let _test_pubb = spawn(move || {
//!         let mut count = 1;
//!         let msg = Message {
//!             header: "dummy".to_owned(),
//!             body:   "hello world".to_owned(),
//!         };
//!
//!         for _ in 0..15 {
//!             let mut msg = msg.clone();
//!             msg.header = format!("{}", count);
//!             let _ = pubb.try_send(msg);
//!
//!             count += 1;
//!         }
//!     });
//!
//!     block_on(sub.take(5).for_each(|e| {
//!         println!("{:?}", e);
//!         ready(())
//!     }));
//!
//!     if let Err(err) = pubsub.shutdown() {
//!         eprintln!("shutdown failure: {:?}", err);
//!     }
//!
//!     Ok(())
//! }
//! ```

#![feature(async_await, await_macro, futures_api)]
#![deny(missing_docs, missing_debug_implementations)]

/// broadcast proposal
pub const PUBSUB_BROADCAST_PROPOSAL: &str = "broadcast_proposal";
/// broadcast vote
pub const PUBSUB_BROADCAST_VOTE: &str = "broadcast_vote";
/// broadcase block
pub const PUBSUB_BROADCAST_BLOCK: &str = "broadcase_block";

/// PubSub builder
pub mod builder;
/// Message channel
pub mod channel;
/// PubSub
pub mod pubsub;
/// Pub/Sub register
pub mod register;

/// Pub message broadcast
pub(crate) mod broadcast;
/// Background worker
pub(crate) mod worker;

/// Re-export
pub use pubsub::PubSub;
