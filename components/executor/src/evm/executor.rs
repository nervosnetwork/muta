use std::cell::RefCell;
use std::convert::From;
use std::str::FromStr;
use std::sync::Arc;

use cita_trie::db::DB as TrieDB;
use cita_vm::{
    evm::{Context as EVMContext, InterpreterResult, Log as EVMLog},
    state::{State, StateObjectInfo},
    BlockDataProvider, Config as EVMConfig, Error as EVMError, Transaction as EVMTransaction,
};
use ethereum_types::{H160, H256, U256};

use core_context::Context;
use core_runtime::{ExecutionResult, Executor, ExecutorError, ReadonlyResult};
use core_types::{
    Address, Balance, BlockHeader, Bloom, BloomInput, Genesis, Hash, LogEntry, Receipt,
    SignedTransaction, StateAlloc, H256 as CoreH256,
};

pub struct EVMExecutor<DB> {
    block_provider: Arc<BlockDataProvider>,

    db: DB,
}

impl<DB> EVMExecutor<DB>
where
    DB: TrieDB,
{
    pub fn from_genesis(
        genesis: &Genesis,
        db: DB,
        block_provider: Arc<BlockDataProvider>,
    ) -> Result<(Self, Hash), ExecutorError> {
        let mut state = State::new(db.clone())?;

        for alloc in genesis.state_alloc.iter() {
            let StateAlloc {
                address,
                code,
                storage,
                balance,
            } = alloc;
            let address = H160::from_str(address).map_err(|e| map_from_str(e.to_string()))?;

            let balance = U256::from_str(balance).map_err(|e| map_from_str(e.to_string()))?;
            state.add_balance(&address, balance)?;
            let code = hex::decode(code).map_err(|e| map_from_str(e.to_string()))?;
            state.set_code(&address, code)?;

            if storage.is_empty() {
                continue;
            }

            for (key, value) in storage {
                state.set_storage(
                    &address,
                    H256::from_str(key).map_err(|e| map_from_str(e.to_string()))?,
                    H256::from_str(value).map_err(|e| map_from_str(e.to_string()))?,
                )?;
            }
        }

        state.commit()?;

        let root_hash = Hash::from_bytes(state.root.as_ref())?;

        let evm_executor = EVMExecutor { block_provider, db };

        Ok((evm_executor, root_hash))
    }

    pub fn from_existing(
        db: DB,
        block_provider: Arc<BlockDataProvider>,
        root: &Hash,
    ) -> Result<Self, ExecutorError> {
        let state_root = H256(root.clone().into_fixed_bytes());
        // Check if state root exists
        State::from_existing(db.clone(), state_root)?;
        Ok(EVMExecutor { block_provider, db })
    }
}

impl<DB: 'static> Executor for EVMExecutor<DB>
where
    DB: TrieDB,
{
    /// Execute the transactions and then return the receipts, this function will modify the "state of the world".
    fn exec(
        &self,
        _: Context,
        latest_state_root: &Hash,
        current_header: &BlockHeader,
        txs: &[SignedTransaction],
    ) -> Result<ExecutionResult, ExecutorError> {
        let state_root = H256(latest_state_root.clone().into_fixed_bytes());

        let state = Arc::new(RefCell::new(State::from_existing(
            self.db.clone(),
            state_root,
        )?));
        let evm_context = build_evm_context(current_header);
        let evm_config = build_evm_config(current_header);

        let mut receipts: Vec<Receipt> = txs
            .iter()
            .map(|signed_tx| {
                EVMExecutor::evm_exec(
                    Arc::clone(&self.block_provider),
                    Arc::clone(&state),
                    &evm_context,
                    &evm_config,
                    &signed_tx,
                )
            })
            .collect();

        state.borrow_mut().commit()?;
        let root_hash = Hash::from_bytes(state.borrow().root.as_ref())?;

        for mut receipt in receipts.iter_mut() {
            receipt.state_root = root_hash.clone();
        }
        let all_logs_bloom = receipts_to_bloom(&receipts);

        Ok(ExecutionResult {
            state_root: root_hash,
            all_logs_bloom,
            receipts,
        })
    }

    /// Query historical height data or perform read-only functions.
    fn readonly(
        &self,
        _: Context,
        header: &BlockHeader,
        to: &Address,
        from: &Address,
        data: &[u8],
    ) -> Result<ReadonlyResult, ExecutorError> {
        let evm_context = build_evm_context(header);
        let evm_config = build_evm_config(header);
        let evm_transaction = build_evm_transaction_of_readonly(to, from, data);

        let root = H256(header.state_root.clone().into_fixed_bytes());
        let state = State::from_existing(self.db.clone(), root)?;

        let result = cita_vm::exec_static(
            Arc::clone(&self.block_provider),
            Arc::new(RefCell::new(state)),
            evm_context,
            evm_config,
            evm_transaction,
        );

        match result {
            Ok(evm_result) => {
                let data = match evm_result {
                    InterpreterResult::Normal(data, _, _) => data,
                    InterpreterResult::Revert(data, _) => data,
                    InterpreterResult::Create(data, _, _, _) => data,
                };
                Ok(ReadonlyResult {
                    data: Some(data),
                    error: None,
                })
            }
            Err(e) => {
                log::error!(target: "evm readonly", "{}", e);
                Ok(ReadonlyResult {
                    data: None,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Query balance of account.
    fn get_balance(
        &self,
        _: Context,
        state_root: &Hash,
        address: &Address,
    ) -> Result<Balance, ExecutorError> {
        let root = H256(state_root.clone().into_fixed_bytes());
        let mut state = State::from_existing(self.db.clone(), root)?;

        let balance = state.balance(&H160::from(address.clone().into_fixed_bytes()))?;
        Ok(to_core_balance(&balance))
    }

    /// Query value of account.
    fn get_value(
        &self,
        _: Context,
        state_root: &Hash,
        address: &Address,
        key: &CoreH256,
    ) -> Result<CoreH256, ExecutorError> {
        let root = H256(state_root.clone().into_fixed_bytes());
        let mut state = State::from_existing(self.db.clone(), root)?;

        let address = &H160::from(address.clone().into_fixed_bytes());
        let key = H256::from(key.clone().into_fixed_bytes());
        let value = state.get_storage(&address, &key)?.to_vec();

        Ok(CoreH256::from_slice(value.as_ref()).unwrap())
    }

    /// Query storage root of account.
    fn get_storage_root(
        &self,
        _: Context,
        state_root: &Hash,
        address: &Address,
    ) -> Result<Hash, ExecutorError> {
        let root = H256(state_root.clone().into_fixed_bytes());
        let mut state = State::from_existing(self.db.clone(), root)?;

        let address = &H160::from(address.clone().into_fixed_bytes());
        let account = state.get_state_object(&address)?;

        let account = account.ok_or(ExecutorError::NotFound)?;
        let storage_root = account.storage_root;
        Hash::from_bytes(storage_root.as_ref()).map_err(ExecutorError::Types)
    }

    /// Query code of account.
    fn get_code(
        &self,
        _: Context,
        state_root: &Hash,
        address: &Address,
    ) -> Result<(Vec<u8>, Hash), ExecutorError> {
        let root = H256(state_root.clone().into_fixed_bytes());
        let mut state = State::from_existing(self.db.clone(), root)?;

        let address = &H160::from(address.clone().into_fixed_bytes());
        let code = state.code(address)?;
        if code.is_empty() {
            return Err(ExecutorError::NotFound);
        }
        let code_hash = state.code_hash(address)?;
        let code_hash = Hash::from_bytes(code_hash.as_ref()).expect("never returns an error");
        Ok((code, code_hash))
    }
}

impl<DB: 'static> EVMExecutor<DB>
where
    DB: TrieDB,
{
    fn evm_exec(
        block_provider: Arc<BlockDataProvider>,
        state: Arc<RefCell<State<DB>>>,
        evm_context: &EVMContext,
        evm_config: &EVMConfig,
        signed_tx: &SignedTransaction,
    ) -> Receipt {
        let account_address = &H160::from(signed_tx.sender.clone().into_fixed_bytes());
        let nonce = match state.borrow_mut().get_state_object(account_address) {
            Ok(opt_account) => {
                if let Some(account) = opt_account {
                    account.nonce
                } else {
                    U256::zero()
                }
            }
            Err(e) => {
                log::error!(target: "evm executor", "{}", e);
                U256::zero()
            }
        };
        let evm_transaction = build_evm_transaction(&signed_tx, nonce);

        let mut receipt = match cita_vm::exec(
            block_provider,
            state,
            evm_context.clone(),
            evm_config.clone(),
            evm_transaction,
        ) {
            Ok(evm_result) => build_receipt_with_ok(signed_tx, evm_result),
            Err(e) => {
                let mut receipt = build_receipt_with_err(e);
                receipt.quota_used = signed_tx.untx.transaction.quota;
                receipt
            }
        };

        receipt.transaction_hash = signed_tx.hash.clone();
        receipt
    }
}

fn build_evm_context(header: &BlockHeader) -> EVMContext {
    EVMContext {
        gas_limit: header.quota_limit,
        coinbase: H160::from(header.proposer.clone().into_fixed_bytes()),
        number: U256::from(header.height),
        timestamp: header.timestamp,

        // The cita-bft consensus does not have difficulty ​​like POW, so set 0
        difficulty: U256::from(0),
    }
}

fn build_evm_config(header: &BlockHeader) -> EVMConfig {
    EVMConfig {
        block_gas_limit: header.quota_limit,
        ..Default::default()
    }
}

fn build_evm_transaction(signed_tx: &SignedTransaction, nonce: U256) -> EVMTransaction {
    let tx = &signed_tx.untx.transaction;
    let from = &signed_tx.sender.clone();
    let value_slice: &[u8] = tx.value.as_ref();
    let to = match &tx.to {
        Some(data) => Some(H160(data.clone().into_fixed_bytes())),
        None => None,
    };

    EVMTransaction {
        from: H160::from(from.clone().into_fixed_bytes()),
        value: U256::from(value_slice),
        gas_limit: tx.quota,
        gas_price: U256::from(1),
        input: tx.data.clone(),
        to,
        nonce,
    }
}

fn build_evm_transaction_of_readonly(to: &Address, from: &Address, data: &[u8]) -> EVMTransaction {
    EVMTransaction {
        to: Some(H160::from(to.clone().into_fixed_bytes())),
        from: H160::from(from.clone().into_fixed_bytes()),
        value: U256::zero(),
        gas_limit: std::u64::MAX,
        gas_price: U256::from(1),
        input: data.to_vec(),
        nonce: U256::zero(),
    }
}

fn build_receipt_with_ok(signed_tx: &SignedTransaction, result: InterpreterResult) -> Receipt {
    let mut receipt = Receipt::default();
    let quota = signed_tx.untx.transaction.quota;

    match result {
        InterpreterResult::Normal(_data, quota_used, logs) => {
            receipt.quota_used = quota - quota_used;
            receipt.logs = transform_logs(logs);
            receipt.logs_bloom = logs_to_bloom(&receipt.logs);
        }
        InterpreterResult::Revert(_data, quota_used) => {
            receipt.quota_used = quota - quota_used;
        }
        InterpreterResult::Create(_data, quota_used, logs, contract_address) => {
            receipt.quota_used = quota - quota_used;
            receipt.logs = transform_logs(logs);
            receipt.logs_bloom = logs_to_bloom(&receipt.logs);

            let address_slice: &[u8] = contract_address.as_ref();
            receipt.contract_address =
                Some(Address::from_bytes(address_slice).expect("never returns an error"));
        }
    };
    receipt
}

fn build_receipt_with_err(err: EVMError) -> Receipt {
    let mut receipt = Receipt::default();
    receipt.receipt_error = err.to_string();
    receipt
}

fn transform_logs(logs: Vec<EVMLog>) -> Vec<LogEntry> {
    logs.into_iter()
        .map(|log| {
            let EVMLog(address, topics, data) = log;

            LogEntry {
                address: Address::from_bytes(address.as_ref()).expect("never returns an error"),
                topics: topics
                    .into_iter()
                    .map(|topic| Hash::from_bytes(topic.as_ref()).expect("never returns an error"))
                    .collect(),
                data,
            }
        })
        .collect()
}

fn receipts_to_bloom(receipts: &[Receipt]) -> Bloom {
    let mut bloom = Bloom::default();

    for receipt in receipts {
        receipt
            .logs
            .iter()
            .for_each(|log| accrue_log(&mut bloom, log));
    }

    bloom
}

fn logs_to_bloom(logs: &[LogEntry]) -> Bloom {
    let mut bloom = Bloom::default();

    logs.iter().for_each(|log| accrue_log(&mut bloom, log));
    bloom
}

fn accrue_log(bloom: &mut Bloom, log: &LogEntry) {
    let address_hash = Hash::digest(log.address.as_bytes());
    let input = BloomInput::Hash(address_hash.as_fixed_bytes());
    bloom.accrue(input);

    for topic in &log.topics {
        let input = BloomInput::Hash(topic.as_fixed_bytes());
        bloom.accrue(input);
    }
}

fn to_core_balance(balance: &U256) -> Balance {
    let mut arr = [0u8; 32];
    balance.to_little_endian(&mut arr);
    Balance::from_little_endian(&arr).unwrap()
}

fn map_from_str(err: String) -> ExecutorError {
    ExecutorError::Internal(err)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use cita_trie::db::MemoryDB;
    use cita_vm::BlockDataProviderMock;

    use core_context::Context;
    use core_crypto::{secp256k1::Secp256k1, Crypto, CryptoTransform};
    use core_runtime::Executor;
    use core_types::{
        Address, Balance, BlockHeader, Genesis, Hash, SignedTransaction, StateAlloc, Transaction,
        UnverifiedTransaction,
    };

    use super::EVMExecutor;

    // pragma solidity ^0.4.24;
    //
    // contract HelloWorld {
    //
    //     string saySomething;
    //     event Print(string out);
    //
    //     constructor() public  {
    //         saySomething = "Hello World!";
    //         Print("Hello, World!");
    //     }
    //
    //     function speak() public constant returns(string itSays) {
    //         return saySomething;
    //     }
    // }
    const CONSTRACT_TEST: &str = "608060405234801561001057600080fd5b506040805190810160405280600c81526020017f48656c6c6f20576f726c642100000000000000000000000000000000000000008152506000908051906020019061005c9291906100ca565b507f241ba3bafc919fb4308284ce03a8f4867a8ec2f0401445d3cf41a468e7db4ae060405180806020018281038252600d8152602001807f48656c6c6f2c20576f726c64210000000000000000000000000000000000000081525060200191505060405180910390a161016f565b828054600181600116156101000203166002900490600052602060002090601f016020900481019282601f1061010b57805160ff1916838001178555610139565b82800160010185558215610139579182015b8281111561013857825182559160200191906001019061011d565b5b509050610146919061014a565b5090565b61016c91905b80821115610168576000816000905550600101610150565b5090565b90565b6101a48061017e6000396000f300608060405260043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff16806350d8531514610046575b600080fd5b34801561005257600080fd5b5061005b6100d6565b6040518080602001828103825283818151815260200191508051906020019080838360005b8381101561009b578082015181840152602081019050610080565b50505050905090810190601f1680156100c85780820380516001836020036101000a031916815260200191505b509250505060405180910390f35b606060008054600181600116156101000203166002900480601f01602080910402602001604051908101604052809291908181526020018280546001816001161561010002031660029004801561016e5780601f106101435761010080835404028352916020019161016e565b820191906000526020600020905b81548152906001019060200180831161015157829003601f168201915b50505050509050905600a165627a7a7230582058a037667a1d48eb9e72da7c03598b2d50402059c3750ea2e6b9a7782ed981dc0029";

    const EMPTY_STATE: &str = "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421";

    #[test]
    fn test_evm_executor_basic() {
        let ctx = Context::new();
        let secp = Secp256k1::new();
        let (_, pubkey) = secp.gen_keypair();
        let pubkey_hash = Hash::digest(&pubkey.as_bytes()[1..]);
        let address = Address::from_hash(&pubkey_hash);
        let genesis = build_genesis(
            address.as_hex(),
            "ffffffffffffffffff".to_owned(),
            "".to_owned(),
            HashMap::default(),
        );

        let (executor, state_root) = EVMExecutor::from_genesis(
            &genesis,
            MemoryDB::new(false),
            Arc::new(BlockDataProviderMock::default()),
        )
        .unwrap();

        // test storage root
        let root = executor
            .get_storage_root(ctx.clone(), &state_root, &address)
            .unwrap();
        assert_eq!(root.as_hex(), EMPTY_STATE);

        // test balance
        let balance = executor
            .get_balance(ctx.clone(), &state_root, &address)
            .unwrap();
        assert_eq!(
            balance,
            Balance::from_hex_str("ffffffffffffffffff").unwrap()
        );
    }

    #[test]
    fn test_create_contract() {
        let ctx = Context::new();
        let secp = Secp256k1::new();
        let (_, pubkey) = secp.gen_keypair();
        let pubkey_hash = Hash::digest(&pubkey.as_bytes()[1..]);
        let address = Address::from_hash(&pubkey_hash);
        let genesis = build_genesis(
            address.as_hex(),
            "ffffffffffffffffff".to_owned(),
            "".to_owned(),
            HashMap::default(),
        );

        let (executor, state_root) = EVMExecutor::from_genesis(
            &genesis,
            MemoryDB::new(false),
            Arc::new(BlockDataProviderMock::default()),
        )
        .unwrap();

        let bin = hex::decode(CONSTRACT_TEST).unwrap();

        let mut header = BlockHeader::default();
        header.quota_limit = 21000 * 100;

        let mut tx = Transaction::default();
        tx.quota = 21000 * 10;
        tx.data = bin;

        let signed_tx = SignedTransaction {
            hash: Hash::digest(b"test1"),
            sender: address,
            untx: UnverifiedTransaction {
                signature: vec![],
                transaction: tx,
            },
        };

        let exec_result = executor
            .exec(ctx, &state_root, &header, &[signed_tx])
            .unwrap();
        assert_ne!(exec_result.receipts[0].contract_address, None);
        assert_ne!(exec_result.state_root, state_root);
        assert_eq!(exec_result.receipts[0].logs.len(), 1);
    }

    fn build_genesis(
        address: String,
        balance: String,
        code: String,
        storage: HashMap<String, String>,
    ) -> Genesis {
        Genesis {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            prevhash: "0000000000000000000000000000".to_owned(),
            state_alloc: vec![StateAlloc {
                code,
                address,
                storage,
                balance,
            }],
        }
    }
}
