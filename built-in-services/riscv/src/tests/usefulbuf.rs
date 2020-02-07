use std::{
    cell::RefCell,
    io::Read,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use protocol::{
    types::{Address, Hash, ServiceContext, ServiceContextParams},
    Bytes,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{mock_context, new_riscv_service, with_dispatcher_service};
use crate::types::{DeployPayload, ExecPayload, InterpreterType};

#[test]
fn test_usefulbuf() {
    let cycles_limit = 0x99_9999; // 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let tx_hash =
        Hash::from_hex("412a6c54cf3d3dbb16b49c34e6cd93d08a245298032eb975ee51105b4c296828").unwrap();
    let nonce =
        Hash::from_hex("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let context = mock_context(cycles_limit, caller, tx_hash, nonce);

    let mut service = new_riscv_service();

    let mut file = std::fs::File::open("src/tests/usefulbuf").unwrap();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();
    let buffer = Bytes::from(buffer);
    let deploy_payload = DeployPayload {
        code:      hex::encode(buffer.as_ref()),
        intp_type: InterpreterType::Binary,
        init_args: "kkk".into(),
    };
    let deploy_result = service.deploy(context.clone(), deploy_payload).unwrap();
    assert_eq!(&deploy_result.init_ret, "");
}
