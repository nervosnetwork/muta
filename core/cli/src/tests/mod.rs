mod service_mapping;

use std::path::PathBuf;
use std::str::FromStr;

use protocol::traits::{CommonStorage, Context};
use protocol::types::{Block, BlockHeader, Bytes, Hash, Proof};
use protocol::ProtocolResult;

use crate::{Cli, CliConfig};

use service_mapping::DefaultServiceMapping;

const SAVE_DIR: &str = "./free-space/save";
const DATA_DIR: &str = "./free-space/data";
const CONFIG_PATH: &str = "./src/tests/config.toml";
const GENESIS_PATH: &str = "./src/tests/genesis.toml";

#[test]
fn test_lineally() {
    clean();

    prepare();
    save_restore();
    clean();

    // set "latest" test before "block" test due to latest block cache in storage
    prepare();
    latest_get(23);
    clean();

    prepare();
    latest_set();
    clean();

    prepare();
    block_get();
    clean();

    prepare();
    block_set();
    clean();
}

fn save_restore() {
    println!("test save_restore");
    let save = PathBuf::from_str(SAVE_DIR).expect("save_restore, path fails");
    fs_extra::dir::remove(save.clone()).expect("save_restore, remove save_restore fails");

    run(vec![
        "muta-chain",
        "--config",
        CONFIG_PATH,
        "--genesis",
        GENESIS_PATH,
        "backup",
        "save",
        SAVE_DIR,
    ])
    .expect("save_restore, run save fails");

    assert!(save.exists());
    // now the data has gone
    clean();

    run(vec![
        "muta-chain",
        "--config",
        CONFIG_PATH,
        "--genesis",
        GENESIS_PATH,
        "backup",
        "restore",
        SAVE_DIR,
    ])
    .expect("save_restore, run restore fails");

    let data = PathBuf::from_str(DATA_DIR).expect("save_restore, path fails");
    assert!(data.exists());

    fs_extra::dir::remove(save).expect("save_restore, remove save files fails");
    println!("tested save_restore");
}

fn block_get() -> Block {
    println!("test block_get");
    let cmd = vec![
        "muta-chain",
        "--config",
        CONFIG_PATH,
        "--genesis",
        GENESIS_PATH,
        "block",
        "get",
        "11",
    ];

    let maintenance_cli = Cli::new(
        DefaultServiceMapping {},
        CliConfig {
            app_name:      "Rodents",
            version:       "Big Cheek",
            author:        "Hamsters",
            config_path:   "./cofnig.toml",
            genesis_patch: "./genesis.toml",
        },
        Some(cmd),
    )
    .generate_maintenance_cli();

    let block = if let ("block", Some(sub_cmd)) = maintenance_cli.matches.subcommand() {
        let mut rt = tokio::runtime::Runtime::new().expect("new tokio runtime");

        if let ("get", Some(_cmd)) = sub_cmd.subcommand() {
            let res = rt.block_on(async move { maintenance_cli.block_get(11).await });
            let block = res
                .expect("block_get, block_get fails")
                .expect("block_get, block_get block not found");
            assert_eq!(block.header.height, 11);
            block
        } else {
            panic!()
        }
    } else {
        panic!()
    };
    println!("tested block_get");
    block
}

fn block_set() {
    println!("test block_set");
    // we chagne the exec height from 10 to 9 on height 11
    let cmd = vec![
        "muta-chain",
        "--config",
        CONFIG_PATH,
        "--genesis",
        GENESIS_PATH,
        "block",
        "set",
        "-y",
        r#"
        {"header":{"chain_id":"0xb6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036","height":11,"exec_height":9,"prev_hash":"0xc60d9652e5a7d18d34272ac4f8350086439520923d812b4cc4428a9b04d2dd01","timestamp":1598632570280,"order_root":"0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421","order_signed_transactions_hash":"0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421","confirm_root":["0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"],"state_root":"0xd26475337965236ee6bfb4db3f02ed8d21b710f4194e7de5a379fdde0f48c681","receipt_root":["0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"],"cycles_used":[0],"proposer":"muta14e0lmgck835vm2dfm0w3ckv6svmez8fdgdl705","proof":{"height":10,"round":0,"block_hash":"0xc60d9652e5a7d18d34272ac4f8350086439520923d812b4cc4428a9b04d2dd01","signature":[7,23,172,129,210,37,136,144,12,57,227,78,29,103,134,41,243,30,237,76,239,6,104,140,72,255,52,0,245,178,160,99,83,172,226,68,115,200,56,126,97,78,80,58,101,70,84,162,8,230,26,25,30,82,91,62,107,140,126,30,95,148,17,78,243,149,82,90,103,206,13,32,42,83,41,233,22,248,127,89,83,246,37,8,152,236,11,120,55,77,110,93,222,191,246,59,11,217,193,133,230,91,73,115,76,124,147,244,154,146,179,147,242,89,239,124,135,95,62,70,190,42,220,245,155,74,210,75,166,138,78,42,247,71,229,134,245,53,10,57,65,253,178,238,14,108,79,191,45,140,142,134,251,157,255,148,122,78,167,127,204,79,176,71,188,253,42,167,34,61,234,242,248,86,0,62,225,11,207,15,254,235,189,202,94,10,185,176,223,127,62,127],"bitmap":[128]},"validator_version":0,"validators":[{"pub_key":[2,239,12,176,215,188,108,24,180,190,161,245,144,141,145,6,82,43,53,171,60,57,147,105,96,93,66,66,82,91,218,126,96],"propose_weight":1,"vote_weight":1}]},"ordered_tx_hashes":[]}
        "#,
    ];

    let maintenance_cli = Cli::new(
        DefaultServiceMapping {},
        CliConfig {
            app_name:      "Rodents",
            version:       "Big Cheek",
            author:        "Hamsters",
            config_path:   "./cofnig.toml",
            genesis_patch: "./genesis.toml",
        },
        Some(cmd),
    )
    .generate_maintenance_cli();
    let mut rt = tokio::runtime::Runtime::new().expect("new tokio runtime");

    rt.block_on(async move {
        if let ("block", Some(sub_cmd)) = maintenance_cli.matches.subcommand() {
            if let ("set", Some(cmd)) = sub_cmd.subcommand() {
                let block_json = cmd.value_of("BLOCK").expect("missing [BLOCK]");

                let res = maintenance_cli.block_set(block_json).await;
                assert!(res.is_ok());
            } else {
                panic!()
            }
        } else {
            panic!()
        }
    });

    let changed = block_get();
    assert_eq!(changed.header.exec_height, 9);
    println!("tested block_set");
}

fn latest_get(expect: u64) -> Block {
    println!("test latest_get");

    // we chagne the exec height from 10 to 9 on height 11
    let cmd = vec![
        "muta-chain",
        "--config",
        CONFIG_PATH,
        "--genesis",
        GENESIS_PATH,
        "latest_block",
        "get",
    ];

    let maintenance_cli = Cli::new(
        DefaultServiceMapping {},
        CliConfig {
            app_name:      "Rodents",
            version:       "Big Cheek",
            author:        "Hamsters",
            config_path:   "./cofnig.toml",
            genesis_patch: "./genesis.toml",
        },
        Some(cmd),
    )
    .generate_maintenance_cli();

    let block = if let ("latest_block", Some(sub_cmd)) = maintenance_cli.matches.subcommand() {
        if let ("get", Some(_cmd)) = sub_cmd.subcommand() {
            let mut rt = tokio::runtime::Runtime::new().expect("new tokio runtime");
            let res = rt.block_on(async move { maintenance_cli.latest_block_get().await });
            let block = res.expect("latest_get, latest_block_get fails");
            assert_eq!(block.header.height, expect);
            block
        } else {
            panic!()
        }
    } else {
        panic!()
    };
    println!("tested latest_get");
    block
}

fn latest_set() {
    println!("test latest_set");

    // we change the exec height from 10 to 9 on height 11
    let cmd = vec![
        "muta-chain",
        "--config",
        CONFIG_PATH,
        "--genesis",
        GENESIS_PATH,
        "latest_block",
        "set",
        "-y",
        "10",
    ];

    let maintenance_cli = Cli::new(
        DefaultServiceMapping {},
        CliConfig {
            app_name:      "Rodents",
            version:       "Big Cheek",
            author:        "Hamsters",
            config_path:   "./cofnig.toml",
            genesis_patch: "./genesis.toml",
        },
        Some(cmd),
    )
    .generate_maintenance_cli();

    if let ("latest_block", Some(sub_cmd)) = maintenance_cli.matches.subcommand() {
        if let ("set", Some(_cmd)) = sub_cmd.subcommand() {
            let mut rt = tokio::runtime::Runtime::new().expect("new tokio runtime");
            let res = rt.block_on(async move { maintenance_cli.latest_block_set(10).await });
            assert!(res.is_ok());
        } else {
            panic!()
        }
    } else {
        panic!()
    }

    let changed = latest_get(10);
    assert_eq!(changed.header.height, 10);
    println!("tested latest_set");
}

// test functional methods list below

fn prepare() {
    let to = PathBuf::from_str(DATA_DIR).expect("prepare,data dir fails");

    if to.exists() {
        fs_extra::dir::remove(to.as_path()).expect("prepare, remove to fails");
    }

    // we just add a validation command, but we don't use the match yet
    let cmd = vec![
        "muta-chain",
        "--config",
        CONFIG_PATH,
        "--genesis",
        GENESIS_PATH,
        "latest_block",
        "get",
    ];

    let maintenance_cli = Cli::new(
        DefaultServiceMapping {},
        CliConfig {
            app_name:      "Rodents",
            version:       "Big Cheek",
            author:        "Hamsters",
            config_path:   "./cofnig.toml",
            genesis_patch: "./genesis.toml",
        },
        Some(cmd),
    )
    .generate_maintenance_cli();

    let storage = maintenance_cli.storage;

    // now we add fake blocks
    let mut rt = tokio::runtime::Runtime::new().expect("new tokio runtime");

    for idx in 0..=23 {
        if let Err(e) = rt.block_on(storage.insert_block(Context::new(), Block {
            header:            BlockHeader {
                chain_id:                       Default::default(),
                height:                         idx,
                exec_height:                    match idx {
                    i if i > 0 => i - 1,
                    _ => 0,
                },
                prev_hash:                      Default::default(),
                timestamp:                      0,
                order_root:                     Default::default(),
                order_signed_transactions_hash: Default::default(),
                confirm_root:                   vec![],
                state_root:                     Default::default(),
                receipt_root:                   vec![],
                cycles_used:                    vec![],
                proposer:                       Default::default(),
                proof:                          Proof {
                    height:     0,
                    round:      0,
                    block_hash: Default::default(),
                    signature:  Default::default(),
                    bitmap:     Default::default(),
                },
                validator_version:              0,
                validators:                     vec![],
            },
            ordered_tx_hashes: vec![],
        })) {
            println!("{:?}", e);
            panic!("muta cli test prepare(), prepare rocksdb fails")
        };
    }

    let tx_wal = maintenance_cli.txs_wal;
    if tx_wal
        .save(
            23,
            Hash::from_hex("0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421")
                .unwrap(),
            vec![],
        )
        .is_err()
    {
        panic!("muta cli test prepare(), prepare tx_wal fails")
    };

    let consensus_wal = maintenance_cli.consensus_wal;

    if consensus_wal
        .update_overlord_wal(
            Context::new(),
            Bytes::from_static(b"1234567,doremifasolati"),
        )
        .is_err()
    {
        panic!("muta cli test prepare(), prepare consense_wal fails")
    };
}

fn clean() {
    let to = PathBuf::from_str(DATA_DIR).expect("clean, data dir fails");
    if to.exists() {
        fs_extra::dir::remove(to.as_path()).expect("clean, remove to");
    }
}

fn run(cmd: Vec<&str>) -> ProtocolResult<()> {
    Cli::new(
        service_mapping::DefaultServiceMapping {},
        CliConfig {
            app_name:      "Rodents",
            version:       "Big Cheek",
            author:        "Hamsters",
            config_path:   "./cofnig.toml",
            genesis_patch: "./genesis.toml",
        },
        Some(cmd),
    )
    .start()
}
