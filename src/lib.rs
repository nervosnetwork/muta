#![feature(async_closure)]

mod config;
mod default_start;

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use derive_more::{Display, From};

use protocol::traits::ServiceMapping;
use protocol::types::{Block, Genesis};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

use crate::config::Config;
use crate::default_start::{create_genesis, start};

#[derive(Default)]
pub struct MutaBuilder<Mapping: ServiceMapping> {
    config_path:     Option<String>,
    genesis_path:    Option<String>,
    servive_mapping: Option<Arc<Mapping>>,
}

impl<Mapping: 'static + ServiceMapping> MutaBuilder<Mapping> {
    pub fn new() -> Self {
        Self {
            servive_mapping: None,
            config_path:     None,
            genesis_path:    None,
        }
    }

    pub fn service_mapping(mut self, mapping: Mapping) -> MutaBuilder<Mapping> {
        self.servive_mapping = Some(Arc::new(mapping));
        self
    }

    pub fn config_path(mut self, path: &str) -> MutaBuilder<Mapping> {
        self.config_path = Some(path.to_owned());
        self
    }

    pub fn genesis_path(mut self, path: &str) -> MutaBuilder<Mapping> {
        self.genesis_path = Some(path.to_owned());
        self
    }

    pub fn build(self) -> ProtocolResult<Muta<Mapping>> {
        let config: Config =
            common_config_parser::parse(&self.config_path.expect("config path is not set"))
                .map_err(MainError::ConfigParse)?;

        let genesis_toml = fs::read_to_string(&self.genesis_path.expect("genesis path is not set"))
            .map_err(MainError::Io)?;
        let genesis: Genesis = toml::from_str(&genesis_toml).map_err(MainError::GenesisTomlDe)?;

        Ok(Muta::new(
            config,
            genesis,
            self.servive_mapping
                .expect("service mapping cannot be None"),
        ))
    }
}

#[derive(Debug, Display)]
#[display(fmt = "exit timeout {}s", "_0.as_secs()")]
struct ExitTimeout(Duration);

impl std::error::Error for ExitTimeout {}

pub struct Muta<Mapping: ServiceMapping> {
    config:          Config,
    genesis:         Genesis,
    service_mapping: Arc<Mapping>,
}

impl<Mapping: 'static + ServiceMapping> Muta<Mapping> {
    pub fn new(config: Config, genesis: Genesis, service_mapping: Arc<Mapping>) -> Self {
        Self {
            config,
            genesis,
            service_mapping,
        }
    }

    pub async fn run(self) -> ProtocolResult<()> {
        common_logger::init(
            self.config.logger.filter.clone(),
            self.config.logger.log_to_console,
            self.config.logger.console_show_file_and_line,
            self.config.logger.log_to_file,
            self.config.logger.metrics,
            self.config.logger.log_path.clone(),
            self.config.logger.modules_level.clone(),
        );

        // run muta
        let mut rt = tokio::runtime::Runtime::new().expect("new tokio runtime");
        let local = tokio::task::LocalSet::new();
        local.block_on(&mut rt, async move {
            self.create_genesis().await?;

            start(self.config, Arc::clone(&self.service_mapping)).await
        })?;

        Ok(())
    }

    async fn create_genesis(&self) -> ProtocolResult<Block> {
        create_genesis(
            &self.config,
            &self.genesis,
            Arc::clone(&self.service_mapping),
        )
        .await
    }
}

#[derive(Debug, Display, From)]
pub enum MainError {
    #[display(fmt = "The muta configuration read failed {:?}", _0)]
    ConfigParse(common_config_parser::ParseError),

    #[display(fmt = "{:?}", _0)]
    Io(std::io::Error),

    #[display(fmt = "Toml fails to parse genesis {:?}", _0)]
    GenesisTomlDe(toml::de::Error),

    #[display(fmt = "hex error {:?}", _0)]
    FromHex(hex::FromHexError),

    #[display(fmt = "crypto error {:?}", _0)]
    Crypto(common_crypto::Error),

    #[display(fmt = "{:?}", _0)]
    Utf8(std::str::Utf8Error),

    #[display(fmt = "other error {:?}", _0)]
    Other(String),
}

impl std::error::Error for MainError {}

impl From<MainError> for ProtocolError {
    fn from(error: MainError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Main, Box::new(error))
    }
}
