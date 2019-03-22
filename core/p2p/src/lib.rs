//! ## Summary
//!
//! Implement core p2p functionality based on
//! [tentacle](https://crates.io/crates/tentacle) crate.

#![deny(missing_docs)]

#[macro_use]
pub(crate) mod macros;

/// Connec protocol
pub mod connec;
/// Discovery protocol
pub mod discovery;
/// Identify protocol
pub mod identify;
/// Ping protocol
pub mod ping;
/// Datagram transport protocol
pub mod transmission;
