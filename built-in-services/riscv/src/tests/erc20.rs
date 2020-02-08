use std::io::Read;

use protocol::{
    types::{Address, Hash},
    Bytes,
};

use super::{mock_context, new_riscv_service};
use crate::types::{DeployPayload, ExecPayload, InterpreterType};

#[test]
fn test_c_erc20() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let tx_hash =
        Hash::from_hex("412a6c54cf3d3dbb16b49c34e6cd93d08a245298032eb975ee51105b4c296828").unwrap();
    let nonce =
        Hash::from_hex("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let context = mock_context(cycles_limit, caller.clone(), tx_hash.clone(), nonce.clone());

    let mut service = new_riscv_service();

    // deploy
    let mut file = std::fs::File::open("examples/dex/erc20.bin").unwrap();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();
    let buffer = Bytes::from(buffer);
    let init_args = serde_json::json!({
        "method": "init",
        "name": "bitcoin",
        "symbol": "BTC",
        "supply": 1_000_000_000,
    })
    .to_string();

    let dep_payoad = DeployPayload {
        code: hex::encode(buffer),
        intp_type: InterpreterType::Binary,
        init_args,
    };
    let address = service
        .deploy(context.clone(), dep_payoad)
        .expect("deploy")
        .address;

    // total supply
    let args = serde_json::json!({
        "method": "total_supply",
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "1000000000".to_owned());

    let args = serde_json::json!({
        "method": "balance_of",
        "account": caller.clone(),
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "1000000000".to_owned());

    let to_address = "0000000000000000000000000000000000000000";
    let args = serde_json::json!({
        "method": "transfer",
        "recipient": to_address,
        "amount": 100,
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "".to_owned());

    let args = serde_json::json!({
        "method": "balance_of",
        "account": caller.clone(),
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "999999900".to_owned());

    let args = serde_json::json!({
        "method": "balance_of",
        "account": to_address,
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "100".to_owned());

    let args = serde_json::json!({
        "method": "approve",
        "spender": to_address,
        "amount": 1000,
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "".to_owned());

    let args = serde_json::json!({
        "method": "allowances",
        "owner": caller.clone(),
        "spender": to_address,
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "1000".to_owned());

    let to_addr2 = "0000000000000000000000000000000000000001";
    let args = serde_json::json!({
        "method": "transfer_from",
        "sender": caller.clone(),
        "amount": 200,
        "recipient": to_addr2,
    })
    .to_string();
    let context2 = mock_context(
        cycles_limit,
        Address::from_hex(to_address).unwrap(),
        tx_hash,
        nonce,
    );
    let exec_ret = service.exec(context2, ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "".to_owned());

    let args = serde_json::json!({
        "method": "allowances",
        "owner": caller.clone(),
        "spender": to_address,
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "800".to_owned());

    let args = serde_json::json!({
        "method": "balance_of",
        "account": caller,
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "999999700".to_owned());

    let args = serde_json::json!({
        "method": "balance_of",
        "account": to_address,
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "100".to_owned());

    let args = serde_json::json!({
        "method": "balance_of",
        "account": to_addr2,
    })
    .to_string();
    let exec_ret = service.exec(context, ExecPayload { address, args });
    assert_eq!(exec_ret.unwrap(), "200".to_owned());
}
