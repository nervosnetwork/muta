mod error;

#[cfg(test)]
mod tests;

use std::fs;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use clap::ArgMatches;
use common_config_parser::types::Config;
use core_consensus::wal::ConsensusWal;
use core_consensus::SignedTxsWAL;
use core_storage::adapter::rocks::RocksAdapter;
use core_storage::ImplStorage;
use protocol::traits::{Context, MaintenanceStorage, ServiceMapping};
use protocol::types::{Block, Genesis, SignedTransaction};
use protocol::ProtocolResult;

use crate::error::CliError;

pub struct Cli<'a, Mapping>
where
    Mapping: 'static + ServiceMapping,
{
    pub matches:         ArgMatches<'a>,
    pub config:          Config,
    pub genesis:         Option<Genesis>,
    pub service_mapping: Arc<Mapping>,
}

impl<'a, Mapping> Cli<'a, Mapping>
where
    Mapping: 'static + ServiceMapping,
{
    pub fn run(service_mapping: Mapping, target_commands: Option<Vec<&str>>) {
        let cli = Self::new(service_mapping, target_commands);
        if let Err(e) = cli.start() {
            log::error!("{:?}", e)
        }
    }

    pub fn new(service_mapping: Mapping, target_commands: Option<Vec<&str>>) -> Self {
        let matches = Self::generate_matches(target_commands);

        let config_path = matches.value_of("config").expect("missing config path");

        let genesis_path = matches.value_of("genesis").expect("missing genesis path");

        let config: Config =
            common_config_parser::parse(&config_path.trim()).expect("config path is not set");

        if !cfg!(test) {
            Self::register_log(&config)
        };

        // genesis may be absent for now
        let genesis = match fs::read_to_string(&genesis_path.trim()) {
            Ok(genesis_content) => match toml::from_str::<Genesis>(&genesis_content) {
                Ok(genesis) => Some(genesis),
                Err(_) => None,
            },
            Err(_) => None,
        };

        Self {
            matches,
            config,
            genesis,
            service_mapping: Arc::new(service_mapping),
        }
    }

    fn register_log(config: &Config) {
        common_logger::init(
            config.logger.filter.clone(),
            config.logger.log_to_console,
            config.logger.console_show_file_and_line,
            config.logger.log_to_file,
            config.logger.metrics,
            config.logger.log_path.clone(),
            config.logger.file_size_limit,
            config.logger.modules_level.clone(),
        );
    }

    pub fn start(self) -> ProtocolResult<()> {
        match self.matches.subcommand() {
            ("run", Some(_sub_cmd)) => {
                log::info!("run subcommand run");
                if let Some(genesis) = self.genesis {
                    let muta = run::Muta::new(self.config, genesis, self.service_mapping);
                    muta.run()
                } else {
                    log::error!("genesis.toml is missing");
                    Err(CliError::MissingGenesis.into())
                }
            }
            ("latest_block", Some(_sub_cmd)) => {
                log::info!("run subcommand latest_block");
                let maintenance_cli = self.generate_maintenance_cli();
                maintenance_cli.start()
            }
            ("block", Some(_sub_cmd)) => {
                log::info!("run subcommand block");
                let maintenance_cli = self.generate_maintenance_cli();
                maintenance_cli.start()
            }

            ("wal", Some(_sub_cmd)) => {
                log::info!("run subcommand wal");
                let maintenance_cli = self.generate_maintenance_cli();
                maintenance_cli.start()
            }

            ("backup", Some(_sub_cmd)) => {
                log::info!("run subcommand backup");
                let maintenance_cli = self.generate_maintenance_cli();
                maintenance_cli.start()
            }
            _ => {
                log::info!("run without any subcommand, default to run");
                if let Some(genesis) = self.genesis {
                    let muta = run::Muta::new(self.config, genesis, self.service_mapping);
                    muta.run()
                } else {
                    log::error!("genesis.toml is missing");
                    Err(CliError::MissingGenesis.into())
                }
            }
        }
    }

    pub fn generate_matches(cmds: Option<Vec<&str>>) -> ArgMatches<'a> {
        let app = clap::App::new("muta-chain")
            .version("v0.2.0-rc.2.1")
            .author("Muta Dev <muta@nervos.org>")
            .arg(
                clap::Arg::with_name("config")
                    .short("c")
                    .long("config")
                    .value_name("FILE")
                    .help("a required file for the configuration")
                    .env("CONFIG")
                    .default_value("./config.toml"),
            )
            .arg(
                clap::Arg::with_name("genesis")
                    .short("g")
                    .long("genesis")
                    .value_name("FILE")
                    .help("a required file for the genesis")
                    .env("GENESIS")
                    .default_value("./genesis.toml"),
            )
            .subcommand(clap::SubCommand::with_name("run").about("run the muta-chain"))
            .subcommand(
                clap::SubCommand::with_name("latest_block")
                    //.help("latest block")
                    .about("APIs for latest block operation")
                    .subcommand(
                        clap::SubCommand::with_name("set")
                            .arg(clap::Arg::with_name("BLOCK_HEIGHT").required(true))
                            .about("set the latest block")
                    )
                    .subcommand(
                        clap::SubCommand::with_name("get")
                            .help("latest_block get")),
            )
            .subcommand(
                clap::SubCommand::with_name("block")
                    .about("APIs for block manipulation")
                    .subcommand(
                        clap::SubCommand::with_name("get")
                            .arg(clap::Arg::with_name("BLOCK_HEIGHT").required(true))
                            .about("get block of [BLOCK_HEIGHT]"),
                    )
                    .subcommand(
                        clap::SubCommand::with_name("set")
                            .arg(clap::Arg::with_name("BLOCK").required(true))
                            .about("upsert target block by [BLOCK], [BLOCK] is in JSON format"),
                    ),
            )
            .subcommand(
                clap::SubCommand::with_name("wal")
                    .about("APIs for Write Ahead Log operation")
                    .subcommand(
                        clap::SubCommand::with_name("clear")
                            .about("clear all wals, include mempool wal and consensus txs"),
                    )
                    .subcommand(
                        clap::SubCommand::with_name("mempool")
                            .about("handle mempool wal")
                            .subcommand(
                                clap::SubCommand::with_name("clear").about("clear mempool wal"),
                            )
                            .subcommand(
                                clap::SubCommand::with_name("list").about("list mempool wal"),
                            )
                            .subcommand(
                                clap::SubCommand::with_name("get")
                                    .about("get mempool wal")
                                    .arg(clap::Arg::with_name("BLOCK_HEIGHT").required(true)),
                            ),
                    )
                    .subcommand(
                        clap::SubCommand::with_name("consensus")
                            .about("handle consensus wal")
                            .subcommand(
                                clap::SubCommand::with_name("clear").about("clear consensus wal"),
                            ),
                    ),
            )
            .subcommand(
                clap::SubCommand::with_name("backup")
                    .about("APIs for backup operation")
                    .subcommand(
                        clap::SubCommand::with_name("save")
                            .about("save db to [TO] place")
                            .arg(clap::Arg::with_name("TO").required(true).help("path")),
                    )
                    .subcommand(
                        clap::SubCommand::with_name("restore")
                            .about("restore db from [FROM] place")
                            .arg(clap::Arg::with_name("FROM").required(true).help("path")),
                    ),
            );
        match cmds {
            Some(cmds) => app.get_matches_from(cmds),
            None => app.get_matches(),
        }
    }

    fn generate_maintenance_cli(self) -> MaintenanceCli<'a, Mapping, ImplStorage<RocksAdapter>> {
        let path_block = self.config.data_path_for_block();
        let rocks_adapter = match RocksAdapter::new(path_block, self.config.rocksdb.max_open_files)
        {
            Ok(adapter) => Arc::new(adapter),
            Err(e) => {
                log::error!("{:?}", e);
                panic!("rocks_adapter init fails")
            }
        };
        let storage = ImplStorage::new(rocks_adapter);

        // Init full transactions wal
        let txs_wal_path = self
            .config
            .data_path_for_txs_wal()
            .to_str()
            .unwrap()
            .to_string();
        let txs_wal = SignedTxsWAL::new(txs_wal_path);

        // Init consensus wal
        let consensus_wal_path = self
            .config
            .data_path_for_consensus_wal()
            .to_str()
            .unwrap()
            .to_string();
        let consensus_wal = ConsensusWal::new(consensus_wal_path);

        MaintenanceCli::new(
            self.matches,
            self.config,
            self.service_mapping,
            storage,
            txs_wal,
            consensus_wal,
        )
    }
}

pub struct MaintenanceCli<'a, Mapping, S>
where
    Mapping: 'static + ServiceMapping,
    S: 'static + MaintenanceStorage,
{
    pub matches:         ArgMatches<'a>,
    pub config:          Config,
    pub service_mapping: Arc<Mapping>,
    pub storage:         Arc<S>,
    pub txs_wal:         Arc<SignedTxsWAL>,
    pub consensus_wal:   Arc<ConsensusWal>,
}

impl<'a, Mapping, S> MaintenanceCli<'a, Mapping, S>
where
    Mapping: 'static + ServiceMapping,
    S: 'static + MaintenanceStorage,
{
    pub fn new(
        matches: ArgMatches<'a>,
        config: Config,
        service_mapping: Arc<Mapping>,
        storage: S,
        txs_wal: SignedTxsWAL,
        consensus_wal: ConsensusWal,
    ) -> Self {
        Self {
            matches,
            config,
            service_mapping,
            storage: Arc::new(storage),
            txs_wal: Arc::new(txs_wal),
            consensus_wal: Arc::new(consensus_wal),
        }
    }

    pub fn start(&self) -> ProtocolResult<()> {
        match self.matches.subcommand() {
            ("latest_block", Some(sub_cmd)) => self.latest_block(sub_cmd),
            ("block", Some(sub_cmd)) => self.block(sub_cmd),
            ("wal", Some(sub_cmd)) => self.wal(sub_cmd),
            ("backup", Some(sub_cmd)) => self.backup(sub_cmd),
            _ => Err(CliError::UnsupportedCommand.into()),
        }
    }

    pub fn latest_block(&self, sub_cmd: &ArgMatches) -> ProtocolResult<()> {
        let mut rt = tokio::runtime::Runtime::new().expect("new tokio runtime");

        match sub_cmd.subcommand() {
            ("set", Some(cmd)) => {
                let height = cmd
                    .value_of("BLOCK_HEIGHT")
                    .expect("missing [BLOCK_HEIGHT]");

                match u64::from_str_radix(height, 10) {
                    Ok(height) => rt.block_on(async move { self.latest_block_set(height).await }),
                    Err(_e) => Err(CliError::Parse.into()),
                }
            }

            ("get", Some(_cmd)) => {
                let block = rt.block_on(async move { self.latest_block_get().await })?;
                log::info!(
                    "latest_block get {}",
                    serde_json::to_string(&block).unwrap()
                );
                Ok(())
            }

            _ => Err(CliError::Grammar.into()),
        }
    }

    pub async fn latest_block_set(&self, height: u64) -> ProtocolResult<()> {
        let last = self.storage.get_latest_block(Context::new()).await?;

        let block = self.block_get(height).await?;
        let block = match block {
            Some(blk) => blk,
            None => return Err(CliError::BlockNotFound(height).into()),
        };

        self.storage
            .insert_block(Context::new(), block.clone())
            .await?;
        log::info!(
            "latest_block set successfully : {}",
            serde_json::to_string(&block).unwrap()
        );

        // now remove 'future' blocks
        for idx in RangeInclusive::new(height + 1, last.header.height) {
            self.storage.remove_block(Context::new(), idx).await?
        }
        log::info!(
            "latest_block set, remove blocks from {} to {}",
            height + 1,
            last.header.height
        );
        Ok(())
    }

    pub async fn latest_block_get(&self) -> ProtocolResult<Block> {
        self.storage.get_latest_block(Context::new()).await
    }

    pub fn block(&self, sub_cmd: &ArgMatches) -> ProtocolResult<()> {
        let mut rt = tokio::runtime::Runtime::new().expect("new tokio runtime");

        match sub_cmd.subcommand() {
            ("set", Some(cmd)) => {
                let block_json = cmd.value_of("BLOCK").expect("missing [BLOCK]");
                rt.block_on(async move { self.block_set(block_json).await })?;

                Ok(())
            }

            ("get", Some(cmd)) => {
                let height = cmd
                    .value_of("BLOCK_HEIGHT")
                    .expect("missing height")
                    .parse()
                    .unwrap();

                let res = rt.block_on(async move { self.block_get(height).await })?;
                match res {
                    Some(block) => {
                        log::info!("block_get: {}", serde_json::to_string(&block).unwrap());
                    }
                    None => {
                        log::info!("block not found for height {}", height);
                    }
                }
                Ok(())
            }

            _ => Err(CliError::Grammar.into()),
        }
    }

    pub async fn block_get(&self, height: u64) -> ProtocolResult<Option<Block>> {
        self.storage.get_block(Context::new(), height).await
    }

    pub async fn block_set(&self, block_json: &str) -> ProtocolResult<()> {
        let block = serde_json::from_str::<Block>(block_json).map_err(|e| {
            log::info!("use 'block get 0' to get a example block JSON output");
            CliError::JSONFormat(e)
        })?;
        self.storage
            .remove_block(Context::new(), block.header.height)
            .await?;
        self.storage
            .set_block(Context::new(), block.clone())
            .await?;
        log::info!(
            "block set successfully: {}",
            serde_json::to_string(&block).unwrap()
        );
        Ok(())
    }

    pub fn wal(&self, sub_cmd: &ArgMatches) -> ProtocolResult<()> {
        match sub_cmd.subcommand() {
            ("mempool", Some(cmd)) => match cmd.subcommand() {
                ("clear", Some(_cmd)) => self.wal_txs_clear(),
                ("list", Some(_cmd)) => {
                    self.wal_txs_list()?;
                    Ok(())
                }
                ("get", Some(cmd)) => {
                    let height = cmd
                        .value_of("BLOCK_HEIGHT")
                        .expect("missing [BLOCK_HEIGHT]")
                        .parse()
                        .unwrap();
                    self.wal_txs_get(height)?;
                    Ok(())
                }
                _ => Err(CliError::Grammar.into()),
            },

            ("consensus", Some(cmd)) => match cmd.subcommand() {
                ("clear", Some(_cmd)) => self.wal_consensus_clear(),
                _ => Err(CliError::Grammar.into()),
            },

            ("clear", Some(_cmd)) => {
                self.wal_consensus_clear()?;
                self.wal_txs_clear()?;
                log::info!("wal clear, successfully");
                Ok(())
            }

            _ => Err(CliError::Grammar.into()),
        }
    }

    pub fn wal_txs_clear(&self) -> ProtocolResult<()> {
        let res = self.txs_wal.remove_all();
        log::info!("wal_txs_clear: {:?}", res);
        res
    }

    pub fn wal_txs_list(&self) -> ProtocolResult<Vec<u64>> {
        let res = self.txs_wal.available_height();
        log::info!("wal_txs_list: {:?}", res);
        res
    }

    pub fn wal_txs_get(&self, height: u64) -> ProtocolResult<Vec<SignedTransaction>> {
        let res = self.txs_wal.load_by_height(height);
        log::info!("wal_txs_get: {:?}", res);
        Ok(res)
    }

    pub fn wal_consensus_clear(&self) -> ProtocolResult<()> {
        let res = self.consensus_wal.clear();
        log::info!("wal_consensus_clear: {:?}", res);
        res
    }

    pub fn backup(&self, sub_cmd: &ArgMatches) -> ProtocolResult<()> {
        match sub_cmd.subcommand() {
            ("save", Some(cmd)) => {
                let to = cmd.value_of("TO").expect("missing [TO]");

                self.backup_save(PathBuf::from_str(to).map_err(|e| CliError::Path(e.to_string()))?)
            }

            ("restore", Some(cmd)) => {
                let from = cmd.value_of("FROM").expect("missing [FROM]");
                self.backup_restore(
                    PathBuf::from_str(from).map_err(|e| CliError::Path(e.to_string()))?,
                )
            }

            _ => Err(CliError::Grammar.into()),
        }
    }

    pub fn backup_save<P: AsRef<Path>>(&self, to: P) -> ProtocolResult<()> {
        let to = to.as_ref();
        let data_path = self.config.data_path.as_path();
        fs_extra::dir::remove(to).map_err(CliError::IO2)?;
        fs_extra::dir::copy(data_path, to, &fs_extra::dir::CopyOptions {
            overwrite:    true,
            skip_exist:   false,
            buffer_size:  64000, // 64kb
            copy_inside:  true,
            content_only: false,
            depth:        0,
        })
        .map_err(CliError::IO2)?;

        log::info!("backup_save successfully to: {:?}", to.to_str());
        Ok(())
    }

    pub fn backup_restore<P: AsRef<Path>>(&self, from: P) -> ProtocolResult<()> {
        let from = from.as_ref();
        let data_path = self.config.data_path.as_path();
        fs_extra::dir::remove(data_path).map_err(CliError::IO2)?;
        fs_extra::dir::copy(from, data_path, &fs_extra::dir::CopyOptions {
            overwrite:    true,
            skip_exist:   false,
            buffer_size:  64000, // 64kb
            copy_inside:  true,
            content_only: false,
            depth:        0,
        })
        .map_err(CliError::IO2)?;
        log::info!("backup_restore successfully to: {:?}", from.to_str());
        Ok(())
    }
}
