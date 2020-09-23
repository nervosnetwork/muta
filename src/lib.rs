#![feature(async_closure)]
#![allow(clippy::mutable_key_type)]

use protocol::traits::ServiceMapping;

use cli::{Cli, CliConfig};

pub fn run<Mapping: 'static + ServiceMapping>(
    service_mapping: Mapping,
    app_name: &'static str,
    version: &'static str,
    author: &'static str,
    config_path: &'static str,
    genesis_patch: &'static str,
    target_commands: Option<Vec<&str>>,
) {
    Cli::run(
        service_mapping,
        CliConfig {
            app_name,
            version,
            author,
            config_path,
            genesis_patch,
        },
        target_commands,
    )
}
