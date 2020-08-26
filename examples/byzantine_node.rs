use std::fs;

use byzantine::config::{Config, Generators};
use byzantine::default_start::start;
use protocol::types::Genesis;

fn main() {
    let config_path =
        std::env::var("CONFIG").unwrap_or_else(|_| "devtools/chain/config.toml".to_owned());
    let genesis_path =
        std::env::var("GENESIS").unwrap_or_else(|_| "devtools/chain/genesis.toml".to_owned());
    let generators_path =
        std::env::var("GENERATORS").unwrap_or_else(|_| "byzantine/generators.toml".to_owned());

    let config: Config = common_config_parser::parse(&config_path).expect("parse config failed");

    let genesis_toml = fs::read_to_string(&genesis_path).expect("read genesis.toml failed");
    let genesis: Genesis = toml::from_str(&genesis_toml).expect("parse genesis failed");

    let generators_toml =
        fs::read_to_string(&generators_path).expect("read generators.toml failed");
    let generators: Generators = toml::from_str(&generators_toml).expect("parse generators failed");

    let mut rt = tokio::runtime::Runtime::new().expect("new tokio runtime");
    let local = tokio::task::LocalSet::new();
    local
        .block_on(
            &mut rt,
            async move { start(config, genesis, generators).await },
        )
        .expect("start failed");
}
