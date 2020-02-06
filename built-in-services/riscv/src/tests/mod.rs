pub mod duktape;

use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use cita_trie::MemoryDB;

use framework::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use framework::binding::state::{GeneralServiceState, MPTTrie};
use protocol::traits::{Dispatcher, ExecResp, Storage};
use protocol::types::{
    Address, Block, Hash, Proof, Receipt, ServiceContext, ServiceContextParams, SignedTransaction,
};
use protocol::{Bytes, ProtocolResult};

use crate::types::{DeployPayload, ExecPayload, InterpreterType};
use crate::RiscvService;

type TestRiscvService = RiscvService<
    DefalutServiceSDK<
        GeneralServiceState<MemoryDB>,
        DefaultChainQuerier<MockStorage>,
        MockDispatcher,
    >,
>;

thread_local! {
    static RISCV_SERVICE: RefCell<TestRiscvService> = RefCell::new(new_riscv_service());
}

fn with_dispatcher_service<R: for<'a> serde::Deserialize<'a>>(
    f: impl FnOnce(&mut TestRiscvService) -> ProtocolResult<R>,
) -> ProtocolResult<R> {
    RISCV_SERVICE.with(|cell| {
        let mut service = cell.borrow_mut();

        f(&mut service)
    })
}

#[test]
fn test_deploy_and_run() {
    let cycles_limit = 0x99_9999; // 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let tx_hash =
        Hash::from_hex("412a6c54cf3d3dbb16b49c34e6cd93d08a245298032eb975ee51105b4c296828").unwrap();
    let nonce =
        Hash::from_hex("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let context = mock_context(cycles_limit, caller, tx_hash, nonce);

    let mut service = new_riscv_service();

    let mut file = std::fs::File::open("src/tests/simple_storage").unwrap();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();
    let buffer = Bytes::from(buffer);
    let deploy_payload = DeployPayload {
        code:      hex::encode(buffer.as_ref()),
        intp_type: InterpreterType::Binary,
        init_args: "set k init".into(),
    };
    let deploy_result = service.deploy(context.clone(), deploy_payload).unwrap();
    assert_eq!(&deploy_result.init_ret, "");

    let address = deploy_result.address;
    let exec_result = service.call(context.clone(), ExecPayload {
        address: address.clone(),
        args:    "get k".into(),
    });
    assert_eq!(&exec_result.unwrap(), "init");
    let exec_payload = ExecPayload {
        address: address.clone(),
        args:    "set k v".into(),
    };
    let exec_result = service.exec(context.clone(), exec_payload);
    assert_eq!(&exec_result.unwrap(), "");
    let exec_result = service.call(context.clone(), ExecPayload {
        address: address.clone(),
        args:    "get k".into(),
    });
    assert_eq!(&exec_result.unwrap(), "v");

    // wrong command
    let exec_result = service.exec(context.clone(), ExecPayload {
        address: address.clone(),
        args:    "clear k v".into(),
    });
    assert!(exec_result.is_err());

    // wrong command 2
    let exec_result = service.exec(context, ExecPayload {
        address,
        args: "set k".into(),
    });
    assert!(exec_result.is_err());
}

struct MockDispatcher;

impl Dispatcher for MockDispatcher {
    fn read(&self, _context: ServiceContext) -> ProtocolResult<ExecResp> {
        unimplemented!()
    }

    fn write(&self, context: ServiceContext) -> ProtocolResult<ExecResp> {
        let payload: ExecPayload =
            serde_json::from_str(context.get_payload()).expect("dispatcher payload");

        RISCV_SERVICE.with(|cell| {
            let mut service = cell.borrow_mut();

            Ok(ExecResp {
                ret:      service.exec(context.clone(), payload)?,
                is_error: false,
            })
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

fn mock_context(cycles_limit: u64, caller: Address, tx_hash: Hash, nonce: Hash) -> ServiceContext {
    let params = ServiceContextParams {
        tx_hash: Some(tx_hash),
        nonce: Some(nonce),
        cycles_limit,
        cycles_price: 1,
        cycles_used: Rc::new(RefCell::new(0)),
        caller,
        height: 1,
        timestamp: 0,
        extra: None,
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

    async fn insert_block(&self, _: Block) -> ProtocolResult<()> {
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

    async fn get_latest_block(&self) -> ProtocolResult<Block> {
        unimplemented!()
    }

    async fn get_block_by_height(&self, _: u64) -> ProtocolResult<Block> {
        unimplemented!()
    }

    async fn get_block_by_hash(&self, _: Hash) -> ProtocolResult<Block> {
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

    async fn update_overlord_wal(&self, _info: Bytes) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn update_muta_wal(&self, _info: Bytes) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn load_overlord_wal(&self) -> ProtocolResult<Bytes> {
        unimplemented!()
    }

    async fn load_muta_wal(&self) -> ProtocolResult<Bytes> {
        unimplemented!()
    }

    async fn update_exec_queue_wal(&self, _info: Bytes) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn load_exec_queue_wal(&self) -> ProtocolResult<Bytes> {
        unimplemented!()
    }

    async fn insert_wal_transactions(
        &self,
        _block_hash: Hash,
        _signed_txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        unimplemented!()
    }

    async fn get_wal_transactions(
        &self,
        _block_hash: Hash,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        unimplemented!()
    }
}
