use super::{mock_context, new_riscv_service};

use protocol::{types::Address, Bytes};

use crate::types::{DeployPayload, ExecPayload, InterpreterType};

#[test]
fn should_able_deploy_js_contract_and_run() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller);

    let mut service = new_riscv_service();

    let test_code = include_str!("./test_code.js");
    let dep_payoad = DeployPayload {
        code:      Bytes::from(test_code),
        intp_type: InterpreterType::Duktape,
        init_args: "args".into(),
    };

    let args = serde_json::json!({
        "x": 5,
        "y": 6
    })
    .to_string();

    let address = service.deploy(context.clone(), dep_payoad).expect("deploy");
    let exec_ret = service.exec(context.clone(), ExecPayload {
        address: Address::from_hex(&address).unwrap(),
        args:    args.into(),
    });

    dbg!(&exec_ret);
}
