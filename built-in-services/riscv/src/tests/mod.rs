pub mod duktape;

use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use cita_trie::MemoryDB;

use framework::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use framework::binding::state::{GeneralServiceState, MPTTrie};
use protocol::traits::{Dispatcher, ExecResp};
use protocol::traits::{NoopDispatcher, Storage};
use protocol::types::{
    Address, Epoch, Hash, Proof, Receipt, ServiceContext, ServiceContextParams, SignedTransaction,
};
use protocol::ProtocolResult;

use crate::types::{DeployPayload, ExecPayload, InterpreterType};
use crate::RiscvService;

#[test]
fn test_deploy_and_run() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller);

    let mut service = new_riscv_service();

    let supply = 1024 * 1024;

    let mut file = std::fs::File::open("src/tests/sys_call").unwrap();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();
    let buffer = bytes::Bytes::from(buffer);
    let deploy_payload = DeployPayload {
        code:      hex::encode(buffer.as_ref()),
        intp_type: InterpreterType::Binary,
        init_args: "args".into(),
    };
    // println!("{}", serde_json::to_string(&deploy_payload).unwrap());
    let address = service.deploy(context.clone(), deploy_payload).unwrap();
    dbg!(&address);
    let exec_payload = ExecPayload {
        address: Address::from_hex(&address).unwrap(),
        args:    "13".into(),
    };
    println!("{}", serde_json::to_string(&exec_payload).unwrap());
    let exec_result = service.exec(context.clone(), exec_payload);
    dbg!(&exec_result);
    assert!(!exec_result.is_err());
    let exec_result = service.exec(context.clone(), ExecPayload {
        address: Address::from_hex(&address).unwrap(),
        args:    "not 13".into(),
    });
    dbg!(&exec_result);
    assert!(!exec_result.is_err());
}

struct MockDispatcher;

impl Dispatcher for MockDispatcher {
    fn read(&self, _context: ServiceContext) -> ProtocolResult<ExecResp> {
        unimplemented!()
    }

    fn write(&self, context: ServiceContext) -> ProtocolResult<ExecResp> {
        dbg!(context);
        Ok(ExecResp {
            ret:      "".to_owned(),
            is_error: false,
        })
    }
}

fn new_riscv_service() -> RiscvService<
    DefalutServiceSDK<
        GeneralServiceState<MemoryDB>,
        DefaultChainQuerier<MockStorage>,
        MockDispatcher,
    >,
> {
    let chain_db = DefaultChainQuerier::new(Arc::new(MockStorage {}));
    let trie = MPTTrie::new(Arc::new(MemoryDB::new(false)));
    let state = GeneralServiceState::new(trie);

    let sdk = DefalutServiceSDK::new(
        Rc::new(RefCell::new(state)),
        Rc::new(chain_db),
        MockDispatcher {},
    );

    RiscvService::init(sdk).unwrap()
}

fn mock_context(cycles_limit: u64, caller: Address) -> ServiceContext {
    let params = ServiceContextParams {
        cycles_limit,
        cycles_price: 1,
        cycles_used: Rc::new(RefCell::new(0)),
        caller,
        epoch_id: 1,
        timestamp: 0,
        service_name: "service_name".to_owned(),
        service_method: "service_method".to_owned(),
        service_payload: "service_payload".to_owned(),
        events: Rc::new(RefCell::new(vec![])),
    };

    ServiceContext::new(params)
}

struct MockStorage;

#[async_trait]
impl Storage for MockStorage {
    async fn insert_transactions(&self, _: Vec<SignedTransaction>) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn insert_epoch(&self, _: Epoch) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn insert_receipts(&self, _: Vec<Receipt>) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn update_latest_proof(&self, _: Proof) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn get_transaction_by_hash(&self, _: Hash) -> ProtocolResult<SignedTransaction> {
        unimplemented!()
    }

    async fn get_transactions(&self, _: Vec<Hash>) -> ProtocolResult<Vec<SignedTransaction>> {
        unimplemented!()
    }

    async fn get_latest_epoch(&self) -> ProtocolResult<Epoch> {
        unimplemented!()
    }

    async fn get_epoch_by_epoch_id(&self, _: u64) -> ProtocolResult<Epoch> {
        unimplemented!()
    }

    async fn get_epoch_by_hash(&self, _: Hash) -> ProtocolResult<Epoch> {
        unimplemented!()
    }

    async fn get_receipt(&self, _: Hash) -> ProtocolResult<Receipt> {
        unimplemented!()
    }

    async fn get_receipts(&self, _: Vec<Hash>) -> ProtocolResult<Vec<Receipt>> {
        unimplemented!()
    }

    async fn get_latest_proof(&self) -> ProtocolResult<Proof> {
        unimplemented!()
    }
}
