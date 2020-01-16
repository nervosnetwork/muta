use std::io::Read;

use protocol::{types::Address, types::Hash, Bytes};

use super::{mock_context, new_riscv_service};
use crate::types::{DeployPayload, ExecPayload, InterpreterType};

#[test]
fn should_able_deploy_js_contract_and_run() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let tx_hash =
        Hash::from_hex("412a6c54cf3d3dbb16b49c34e6cd93d08a245298032eb975ee51105b4c296828").unwrap();
    let nonce =
        Hash::from_hex("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let context = mock_context(cycles_limit, caller, tx_hash, nonce);

    let mut service = new_riscv_service();

    let test_code = include_str!("./test_code.js");
    let dep_payoad = DeployPayload {
        code:      hex::encode(Bytes::from(test_code)),
        intp_type: InterpreterType::Duktape,
        init_args: "{}".into(),
    };

    let bin_test_code = include_bytes!("./sys_call");
    let bin_dep_payload = DeployPayload {
        code:      hex::encode(Bytes::from(bin_test_code.as_ref())),
        intp_type: InterpreterType::Binary,
        init_args: "set k init".into(),
    };

    let args = serde_json::json!({
        "x": 5,
        "y": 6
    })
    .to_string();

    service
        .deploy(context.clone(), bin_dep_payload)
        .expect("deplay binary");

    let address = service
        .deploy(context.clone(), dep_payoad)
        .expect("deploy")
        .address;

    let exec_ret = service.exec(context.clone(), ExecPayload {
        address,
        args: args.into(),
    });

    dbg!(&exec_ret);
}

#[test]
fn test_js_erc20() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let tx_hash =
        Hash::from_hex("412a6c54cf3d3dbb16b49c34e6cd93d08a245298032eb975ee51105b4c296828").unwrap();
    let nonce =
        Hash::from_hex("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let context = mock_context(cycles_limit, caller, tx_hash, nonce);

    let mut service = new_riscv_service();

    // deploy
    let mut file = std::fs::File::open("src/tests/erc20.js").unwrap();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();
    let buffer = bytes::Bytes::from(buffer);
    let init_args = serde_json::json!({
        "method": "init",
        "name": "bitcoin",
        "symbol": "BTC",
        "supply": 1000000000,
    })
    .to_string();
    dbg!(&init_args);
    let dep_payoad = DeployPayload {
        code: hex::encode(buffer),
        intp_type: InterpreterType::Duktape,
        init_args,
    };
    let address = service
        .deploy(context.clone(), dep_payoad)
        .expect("deploy")
        .address;

    // total supply
    let address_hex = &address.as_hex();
    let args = serde_json::json!({
        "method": "total_supply",
    })
    .to_string();
    let exec_ret = service.call(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    dbg!(&exec_ret);

    let address_hex = &address.as_hex();
    let to_address = "0000000000000000000000000000000000000000";
    let args = serde_json::json!({
        "method": "transfer",
        "recipient": to_address,
        "amount": 100,
    })
    .to_string();
    let exec_ret = service.call(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    dbg!(&exec_ret);

    let args = serde_json::json!({
        "method": "balance_of",
        "account": to_address,
    })
    .to_string();
    let exec_ret = service.call(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    dbg!(&exec_ret);
}
