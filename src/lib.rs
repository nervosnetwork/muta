#![feature(async_closure)]
#![allow(clippy::mutable_key_type)]

use protocol::traits::ServiceMapping;

use cli::Cli;

pub fn run<Mapping: 'static + ServiceMapping>(
    service_mapping: Mapping,
    target_commands: Option<Vec<&str>>,
) {
    Cli::run(service_mapping, target_commands)
}
