use std::{
    cell::RefCell,
    io::Read,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use protocol::{
    types::{Address, Hash, ServiceContext, ServiceContextParams},
    Bytes,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{mock_context, new_riscv_service};
use crate::types::{DeployPayload, ExecPayload, InterpreterType};

const CYCLE_LIMIT: u64 = 1024 * 1024 * 1024;

struct TestContext {
    count: usize,
}

impl Default for TestContext {
    fn default() -> Self {
        TestContext { count: 1 }
    }
}

impl TestContext {
    fn make(&mut self) -> ServiceContext {
        self.count += 1;

        let caller = "0x0000000000000000000000000000000000000001";
        let tx_hash = Hash::digest(Bytes::from(format!("{}", self.count)));

        let params = ServiceContextParams {
            tx_hash:         Some(tx_hash),
            nonce:           None,
            cycles_limit:    CYCLE_LIMIT,
            cycles_price:    1,
            cycles_used:     Rc::new(RefCell::new(0)),
            caller:          Address::from_hex(caller).expect("ctx caller"),
            epoch_id:        1,
            timestamp:       0,
            extra:           None,
            service_name:    "service_name".to_owned(),
            service_method:  "service_method".to_owned(),
            service_payload: "service_payload".to_owned(),
            events:          Rc::new(RefCell::new(vec![])),
        };

        ServiceContext::new(params)
    }
}

macro_rules! deploy_test_code {
    () => {{
        let mut context = TestContext::default();
        let mut service = new_riscv_service();

        let code = include_str!("./test_code.js");
        let payload = DeployPayload {
            code:      hex::encode(Bytes::from(code)),
            intp_type: InterpreterType::Duktape,
            init_args: "{}".into(),
        };

        let ret = service.deploy(context.make(), payload).expect("deploy");
        assert_eq!(ret.init_ret, "init");

        (service, context, ret.address)
    }};
}

#[test]
fn should_support_pvm_init() {
    let (mut service, mut context, address) = deploy_test_code!();

    let args = json!({"method": "test_init"}).to_string();
    let payload = ExecPayload::new(address, args.into());

    let ret = service.exec(context.make(), payload).expect("init");

    assert_eq!("not init", ret);
}

#[test]
fn should_support_pvm_load_args() {
    let (mut service, mut context, address) = deploy_test_code!();

    let args = json!({"method": "test_load_args"}).to_string();
    let payload = ExecPayload::new(address, args.clone().into());

    let ret = service.exec(context.make(), payload).expect("load args");

    assert_eq!(ret, args);
}

#[test]
fn should_support_pvm_load_json_args() {
    let (mut service, mut context, address) = deploy_test_code!();

    let args = json!({"method": "test_load_json_args"}).to_string();
    let payload = ExecPayload::new(address, args.clone().into());

    let ret = service
        .exec(context.make(), payload)
        .expect("load jsonn args");

    assert_eq!(ret, args);
}

#[test]
fn should_support_pvm_cycle_limit() {
    let (mut service, mut context, address) = deploy_test_code!();

    let args = json!({"method": "test_cycle_limit"}).to_string();
    let payload = ExecPayload::new(address, args.clone().into());

    let ret = service
        .exec(context.make(), payload)
        .expect("load cycle limit");

    assert_eq!(ret.parse::<u64>().expect("cycle limit"), CYCLE_LIMIT);
}

#[test]
fn should_support_pvm_storage() {
    let (mut service, mut context, address) = deploy_test_code!();

    #[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
    struct Carmen {
        color: String,
    }

    let carmen = json!({"color": "red"}).to_string();
    let args = json!({"method": "test_storage", "key": "carmen", "val": carmen}).to_string();
    let payload = ExecPayload::new(address, args.clone().into());

    let ret = service.exec(context.make(), payload).expect("load storage");

    let ret: Carmen = serde_json::from_str(&ret).expect("get json storage");

    assert_eq!(ret.color, "red");
}

#[test]
fn should_support_pvm_contract_call() {
    let (mut service, mut context, address) = deploy_test_code!();

    // Deploy another contract
    let simple_storage = include_bytes!("./simple_storage");
    let payload = DeployPayload {
        code:      hex::encode(Bytes::from(simple_storage.as_ref())),
        intp_type: InterpreterType::Binary,
        init_args: "set k carmen".into(),
    };
    let ss_ret = service
        .deploy(context.make(), payload)
        .expect("deploy simple storage");

    let args =
        json!({"method": "test_contract_call", "address": ss_ret.address.as_hex(), "call_args": "get k"})
            .to_string();

    let payload = ExecPayload::new(address.clone(), args.into());

    let ret = service
        .exec(context.make(), payload)
        .expect("call contract get k");

    assert_eq!(ret, "carmen");
}

#[test]
fn test_js_erc20() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let tx_hash =
        Hash::from_hex("412a6c54cf3d3dbb16b49c34e6cd93d08a245298032eb975ee51105b4c296828").unwrap();
    let nonce =
        Hash::from_hex("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let context = mock_context(cycles_limit, caller.clone(), tx_hash.clone(), nonce.clone());

    let mut service = new_riscv_service();

    // deploy
    let mut file = std::fs::File::open("examples/dex/erc20.js").unwrap();
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

    let address_hex = &address.as_hex();
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
        "account": caller.clone(),
    })
    .to_string();
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "999999700".to_owned());

    let args = serde_json::json!({
        "method": "balance_of",
        "account": to_address.clone(),
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
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args,
    });
    assert_eq!(exec_ret.unwrap(), "200".to_owned());
}
