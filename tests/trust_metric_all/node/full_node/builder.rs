use super::{
    config::Config,
    default_start::{create_genesis, start},
    error::MainError,
    memory_db::MemoryDB,
};

use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use protocol::traits::ServiceMapping;
use protocol::types::{Block, Genesis};
use protocol::ProtocolResult;

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

    pub fn build(self, listen_port: u16) -> ProtocolResult<Muta<Mapping>> {
        let mut config: Config =
            common_config_parser::parse(&self.config_path.expect("config path is not set"))
                .map_err(MainError::ConfigParse)?;

        // Override listening address
        let listen_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), listen_port);
        config.network.listening_address = listen_addr;

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

    pub fn run(self) -> ProtocolResult<()> {
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
        let memory_db = MemoryDB::default();

        local.block_on(&mut rt, async move {
            self.create_genesis(memory_db.clone()).await?;

            start(self.config, Arc::clone(&self.service_mapping), memory_db).await
        })?;

        Ok(())
    }

    async fn create_genesis(&self, db: MemoryDB) -> ProtocolResult<Block> {
        create_genesis(&self.genesis, Arc::clone(&self.service_mapping), db).await
    }
}
