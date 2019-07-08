use std::cell::RefCell;
use std::convert::From;
use std::str::FromStr;
use std::sync::Arc;

use cita_trie::DB as TrieDB;
use cita_vm::{
    evm::{Context as EVMContext, InterpreterResult, Log as EVMLog},
    state::{State, StateObjectInfo},
    BlockDataProvider, Config as EVMConfig, Error as EVMError, Transaction as EVMTransaction,
};
use ethereum_types::{H160, H256, U256};
use parking_lot::RwLock;

use core_context::Context;
use core_runtime::{ExecutionContext, ExecutionResult, Executor, ExecutorError, ReadonlyResult};
use core_types::{
    Address, Balance, Bloom, BloomInput, Genesis, Hash, LogEntry, Receipt, SignedTransaction,
    StateAlloc, H256 as CoreH256, U256 as CoreU256,
};

use crate::evm::config::{EconomicsModel, ExecutorConfig};

pub struct EVMExecutor<DB> {
    config: RwLock<ExecutorConfig>,

    block_provider: Arc<BlockDataProvider>,

    db: Arc<DB>,
}

impl<DB> EVMExecutor<DB>
where
    DB: TrieDB,
{
    pub fn from_genesis(
        genesis: &Genesis,
        db: Arc<DB>,
        block_provider: Arc<BlockDataProvider>,
    ) -> Result<(Self, Hash), ExecutorError> {
        let mut state = State::new(Arc::<DB>::clone(&db))?;

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

        let config = RwLock::new(ExecutorConfig::default());
        let evm_executor = EVMExecutor {
            block_provider,
            db,
            config,
        };

        Ok((evm_executor, root_hash))
    }

    pub fn from_existing(
        db: Arc<DB>,
        block_provider: Arc<BlockDataProvider>,
        root: &Hash,
    ) -> Result<Self, ExecutorError> {
        let state_root = H256(root.clone().into_fixed_bytes());
        // Check if state root exists
        State::from_existing(Arc::<DB>::clone(&db), state_root)?;
        let config = RwLock::new(ExecutorConfig::default());
        Ok(EVMExecutor {
            block_provider,
            db,
            config,
        })
    }

    pub fn set_config(&self, config: ExecutorConfig) {
        *self.config.write() = config
    }
}

impl<DB: 'static> Executor for EVMExecutor<DB>
where
    DB: TrieDB,
{
    /// Execute the transactions and then return the receipts, this function
    /// will modify the "state of the world".
    fn exec(
        &self,
        _: Context,
        execution_ctx: &ExecutionContext,
        txs: &[SignedTransaction],
    ) -> Result<ExecutionResult, ExecutorError> {
        let state_root = H256(execution_ctx.state_root.clone().into_fixed_bytes());

        let state = Arc::new(RefCell::new(State::from_existing(
            Arc::<DB>::clone(&self.db),
            state_root,
        )?));
        let mut coinbase = None;
        if let EconomicsModel::Charge(charge_config) = &self.config.read().economics_model {
            if let Some(addr) = charge_config.coinbase {
                coinbase = Some(addr);
            }
        }
        let evm_context = build_evm_context(execution_ctx, coinbase);
        let evm_config = build_evm_config(execution_ctx);

        let mut receipts: Vec<Receipt> = txs
            .iter()
            .map(|signed_tx| {
                self.evm_exec(
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
        execution_ctx: &ExecutionContext,
        to: &Address,
        from: &Address,
        data: &[u8],
    ) -> Result<ReadonlyResult, ExecutorError> {
        let evm_context = build_evm_context(execution_ctx, None);
        let evm_config = build_evm_config(execution_ctx);
        let evm_transaction = build_evm_transaction_of_readonly(to, from, data);

        let root = H256(execution_ctx.state_root.clone().into_fixed_bytes());
        let state = State::from_existing(Arc::<DB>::clone(&self.db), root)?;

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
                    data:  Some(data),
                    error: None,
                })
            }
            Err(e) => {
                log::error!(target: "evm readonly", "{}", e);
                Ok(ReadonlyResult {
                    data:  None,
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
        let mut state = State::from_existing(Arc::<DB>::clone(&self.db), root)?;

        let balance = state.balance(&H160::from(address.clone().into_fixed_bytes()))?;
        Ok(to_core_balance(&balance))
    }

    /// Query nonce of account.
    fn get_nonce(
        &self,
        _: Context,
        state_root: &Hash,
        address: &Address,
    ) -> Result<CoreU256, ExecutorError> {
        let root = H256(state_root.clone().into_fixed_bytes());
        let mut state = State::from_existing(Arc::<DB>::clone(&self.db), root)?;

        let nonce = state.nonce(&H160::from(address.clone().into_fixed_bytes()))?;
        Ok(to_core_u256(&nonce))
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
        let mut state = State::from_existing(Arc::<DB>::clone(&self.db), root)?;

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
        let state = State::from_existing(Arc::<DB>::clone(&self.db), root)?;

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
        let mut state = State::from_existing(Arc::<DB>::clone(&self.db), root)?;

        let address = &H160::from(address.clone().into_fixed_bytes());
        let code = state.code(address)?;
        if code.is_empty() {
            return Err(ExecutorError::NotFound);
        }
        let code_hash = state.code_hash(address)?;
        let code_hash = Hash::from_bytes(code_hash.as_ref()).expect("never returns an error");
        Ok((code, code_hash))
    }

    /// Get the merkle proof for a given account.
    fn get_account_proof(
        &self,
        _: Context,
        state_root: &Hash,
        address: &Address,
    ) -> Result<Vec<Vec<u8>>, ExecutorError> {
        let root = H256(state_root.clone().into_fixed_bytes());
        let state = State::from_existing(Arc::<DB>::clone(&self.db), root)?;
        let address = &H160::from(address.clone().into_fixed_bytes());
        let proof = state.get_account_proof(address)?;
        Ok(proof)
    }

    /// Get the storage proof for given account and key.
    fn get_storage_proof(
        &self,
        _: Context,
        state_root: &Hash,
        address: &Address,
        key: &Hash,
    ) -> Result<Vec<Vec<u8>>, ExecutorError> {
        let root = H256(state_root.clone().into_fixed_bytes());
        let state = State::from_existing(Arc::<DB>::clone(&self.db), root)?;
        let key = H256(key.clone().into_fixed_bytes());
        let address = &H160::from(address.clone().into_fixed_bytes());
        let proof = state.get_storage_proof(address, &key)?;
        Ok(proof)
    }
}

impl<DB: 'static> EVMExecutor<DB>
where
    DB: TrieDB,
{
    fn evm_exec(
        &self,
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
        let gas_price = match &self.config.read().economics_model {
            EconomicsModel::Quota => 0,
            EconomicsModel::Charge(charge_config) => charge_config.gas_price,
        };
        let evm_transaction = build_evm_transaction(&signed_tx, nonce, gas_price);

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

fn build_evm_context(ctx: &ExecutionContext, coinbase: Option<H160>) -> EVMContext {
    let coinbase = coinbase.unwrap_or_else(|| H160::from(ctx.proposer.clone().into_fixed_bytes()));
    EVMContext {
        gas_limit: ctx.quota_limit,
        coinbase,
        number: U256::from(ctx.height),
        timestamp: ctx.timestamp,

        // The cita-bft consensus does not have difficulty ​​like POW, so set 0
        difficulty: U256::from(0),
    }
}

fn build_evm_config(ctx: &ExecutionContext) -> EVMConfig {
    EVMConfig {
        block_gas_limit: ctx.quota_limit,
        ..Default::default()
    }
}

fn build_evm_transaction(
    signed_tx: &SignedTransaction,
    nonce: U256,
    gas_price: u64,
) -> EVMTransaction {
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
        gas_price: U256::from(gas_price),
        input: tx.data.clone(),
        to,
        nonce,
    }
}

fn build_evm_transaction_of_readonly(to: &Address, from: &Address, data: &[u8]) -> EVMTransaction {
    EVMTransaction {
        to:        Some(H160::from(to.clone().into_fixed_bytes())),
        from:      H160::from(from.clone().into_fixed_bytes()),
        value:     U256::zero(),
        gas_limit: std::u64::MAX,
        gas_price: U256::from(1),
        input:     data.to_vec(),
        nonce:     U256::zero(),
    }
}

fn build_receipt_with_ok(signed_tx: &SignedTransaction, result: InterpreterResult) -> Receipt {
    let mut receipt = Receipt::default();
    let quota = signed_tx.untx.transaction.quota;

    match result {
        InterpreterResult::Normal(_data, quota_left, logs) => {
            receipt.quota_used = quota - quota_left;
            receipt.logs = transform_logs(logs);
            receipt.logs_bloom = logs_to_bloom(&receipt.logs);
        }
        InterpreterResult::Revert(_data, quota_left) => {
            receipt.quota_used = quota - quota_left;
        }
        InterpreterResult::Create(_data, quota_left, logs, contract_address) => {
            receipt.quota_used = quota - quota_left;
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
    bloom.accrue(BloomInput::Raw(log.address.as_bytes()));

    for topic in &log.topics {
        let input = BloomInput::Hash(topic.as_fixed_bytes());
        bloom.accrue(input);
    }
}

fn to_core_balance(balance: &U256) -> Balance {
    to_core_u256(balance) as Balance
}

fn to_core_u256(u: &U256) -> CoreU256 {
    let mut arr = [0u8; 32];
    u.to_little_endian(&mut arr);
    CoreU256::from_little_endian(&arr).unwrap()
}

fn map_from_str(err: String) -> ExecutorError {
    ExecutorError::Internal(err)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use cita_trie::MemoryDB;
    use cita_vm::BlockDataProviderMock;
    use ethereum_types::Address as EthAddress;

    use core_context::Context;
    use core_crypto::{secp256k1::Secp256k1, Crypto, CryptoTransform};
    use core_runtime::{ExecutionContext, Executor};
    use core_types::{
        Address, Balance, BlockHeader, Genesis, Hash, SignedTransaction, StateAlloc, Transaction,
        UnverifiedTransaction,
    };

    use crate::evm::config::{ChargeConfig, EconomicsModel, ExecutorConfig};

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

    // pragma solidity ^0.4.24;

    // contract SimpleStorage {
    //     uint storedData;

    //     function set(uint x) public {
    //         storedData = x;
    //     }

    //     function get() view public returns (uint) {
    //         return storedData;
    //     }
    // }
    const SIMPLE_STORAGE_CONTRACT: &str = "608060405234801561001057600080fd5b5060df8061001f6000396000f3006080604052600436106049576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff16806360fe47b114604e5780636d4ce63c146078575b600080fd5b348015605957600080fd5b5060766004803603810190808035906020019092919050505060a0565b005b348015608357600080fd5b50608a60aa565b6040518082815260200191505060405180910390f35b8060008190555050565b600080549050905600a165627a7a7230582099c66a25d59f0aa78f7ebc40748fa1d1fbc335d8d780f284841b30e0365acd960029";

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
            Arc::new(MemoryDB::new(false)),
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
            Arc::new(MemoryDB::new(false)),
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
            hash:   Hash::digest(b"test1"),
            sender: address,
            untx:   UnverifiedTransaction {
                signature:   vec![],
                transaction: tx,
            },
        };

        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let exec_result = executor.exec(ctx, &execution_ctx, &[signed_tx]).unwrap();
        assert_ne!(exec_result.receipts[0].contract_address, None);
        assert_ne!(exec_result.state_root, state_root);
        assert_eq!(exec_result.receipts[0].logs.len(), 1);
    }

    #[test]
    fn test_exec_contract() {
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
            Arc::new(MemoryDB::new(false)),
            Arc::new(BlockDataProviderMock::default()),
        )
        .unwrap();

        let bin = hex::decode(SIMPLE_STORAGE_CONTRACT).unwrap();

        let mut header = BlockHeader::default();
        header.quota_limit = 21000 * 100;

        let mut tx = Transaction::default();
        tx.quota = 21000 * 10;
        tx.data = bin;

        let signed_tx = SignedTransaction {
            hash:   Hash::digest(b"test1"),
            sender: address.clone(),
            untx:   UnverifiedTransaction {
                signature:   vec![],
                transaction: tx,
            },
        };

        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let exec_result = executor.exec(ctx, &execution_ctx, &[signed_tx]).unwrap();
        // dbg!(&exec_result);
        assert_ne!(exec_result.state_root, state_root);
        let contract_addr = exec_result.receipts[0].contract_address.clone().unwrap();
        let state_root = &exec_result.receipts[0].state_root;

        // call get method
        let data = hex::decode("6d4ce63c").unwrap(); // get()
        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let ctx = Context::new();
        let exec_result = executor.readonly(ctx, &execution_ctx, &contract_addr, &address, &data);
        // dbg!(&exec_result);
        assert_eq!(exec_result.unwrap().data.unwrap(), [0; 32]);

        // call set method
        let mut tx = Transaction::default();
        tx.quota = 21000 * 10;
        tx.data =
            hex::decode("60fe47b10000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(); // set(1)
        tx.to = Some(contract_addr.clone());
        let signed_tx = SignedTransaction {
            hash:   Hash::digest(b"test1"),
            sender: address.clone(),
            untx:   UnverifiedTransaction {
                signature:   vec![],
                transaction: tx,
            },
        };
        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let ctx = Context::new();
        let exec_result = executor.exec(ctx, &execution_ctx, &[signed_tx]).unwrap();
        // dbg!(&exec_result);
        let state_root = &exec_result.receipts[0].state_root;

        // call get method
        let data = hex::decode("6d4ce63c").unwrap(); // get() method
        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let ctx = Context::new();
        let exec_result = executor.readonly(ctx, &execution_ctx, &contract_addr, &address, &data);
        // dbg!(&exec_result);
        assert_eq!(exec_result.unwrap().data.unwrap(), [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1
        ]);
    }

    #[test]
    fn test_business_model() {
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
            Arc::new(MemoryDB::new(false)),
            Arc::new(BlockDataProviderMock::default()),
        )
        .unwrap();

        let bin = hex::decode(SIMPLE_STORAGE_CONTRACT).unwrap();

        let mut header = BlockHeader::default();
        header.quota_limit = 21000 * 100;

        let mut tx = Transaction::default();
        tx.quota = 21000 * 10;
        tx.data = bin;

        let signed_tx = SignedTransaction {
            hash:   Hash::digest(b"test1"),
            sender: address.clone(),
            untx:   UnverifiedTransaction {
                signature:   vec![],
                transaction: tx,
            },
        };

        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let exec_result = executor
            .exec(Context::new(), &execution_ctx, &[signed_tx])
            .unwrap();
        // dbg!(&exec_result);
        assert_ne!(exec_result.state_root, state_root);
        let contract_addr = exec_result.receipts[0].contract_address.clone().unwrap();
        let state_root = &exec_result.receipts[0].state_root;
        let balance1 = executor.get_balance(ctx, &state_root, &address).unwrap();
        // dbg!(balance1);

        // call set method
        // use default config, Charge mode, price is 1, and coinbase is none
        let mut tx = Transaction::default();
        tx.quota = 21000 * 10;
        tx.data =
            hex::decode("60fe47b10000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(); // set(1)
        tx.to = Some(contract_addr.clone());
        let signed_tx = SignedTransaction {
            hash:   Hash::digest(b"test1"),
            sender: address.clone(),
            untx:   UnverifiedTransaction {
                signature:   vec![],
                transaction: tx.clone(),
            },
        };
        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let exec_result = executor
            .exec(Context::new(), &execution_ctx, &[signed_tx.clone()])
            .unwrap();
        let state_root = &exec_result.receipts[0].state_root;
        let balance2 = executor
            .get_balance(Context::new(), &state_root, &address)
            .unwrap();
        let balance_diff = balance1 - balance2.clone();
        assert_eq!(balance_diff, exec_result.receipts[0].quota_used.into());

        // set price to 2
        let config = ExecutorConfig {
            economics_model: EconomicsModel::Charge(ChargeConfig {
                gas_price: 2,
                coinbase:  None,
            }),
        };
        executor.set_config(config);
        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let exec_result = executor
            .exec(Context::new(), &execution_ctx, &[signed_tx.clone()])
            .unwrap();
        let state_root = &exec_result.receipts[0].state_root;
        let balance3 = executor
            .get_balance(Context::new(), &state_root, &address)
            .unwrap();
        let balance_diff = balance2 - balance3.clone();
        assert_eq!(
            balance_diff,
            (exec_result.receipts[0].quota_used * 2).into()
        );

        // set coinbase
        let coinbase_addr = Address::from_bytes(&[17; 20]).unwrap();
        let coinbase_eth_addr = EthAddress::from("0x1111111111111111111111111111111111111111");
        let balance4 = executor
            .get_balance(Context::new(), &state_root, &coinbase_addr)
            .unwrap();
        assert_eq!(balance4, 0u64.into());
        let config = ExecutorConfig {
            economics_model: EconomicsModel::Charge(ChargeConfig {
                gas_price: 3,
                coinbase:  Some(coinbase_eth_addr),
            }),
        };
        executor.set_config(config);
        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let exec_result = executor
            .exec(Context::new(), &execution_ctx, &[signed_tx.clone()])
            .unwrap();
        let state_root = &exec_result.receipts[0].state_root;
        let balance5 = executor
            .get_balance(Context::new(), &state_root, &address)
            .unwrap();
        let balance_diff = balance3 - balance5.clone();
        assert_eq!(
            balance_diff,
            (exec_result.receipts[0].quota_used * 3).into()
        );
        let balance6 = executor
            .get_balance(Context::new(), &state_root, &coinbase_addr)
            .unwrap();
        assert_eq!(&balance6, &balance_diff);

        // set mode to quota
        let config = ExecutorConfig {
            economics_model: EconomicsModel::Quota,
        };
        executor.set_config(config);
        let execution_ctx = ExecutionContext {
            state_root:  state_root.clone(),
            proposer:    header.proposer.clone(),
            height:      header.height,
            quota_limit: header.quota_limit,
            timestamp:   header.timestamp,
        };
        let exec_result = executor
            .exec(Context::new(), &execution_ctx, &[signed_tx.clone()])
            .unwrap();
        let state_root = &exec_result.receipts[0].state_root;
        let balance7 = executor
            .get_balance(Context::new(), &state_root, &address)
            .unwrap();
        assert_eq!(&balance7, &balance5);
    }

    fn build_genesis(
        address: String,
        balance: String,
        code: String,
        storage: HashMap<String, String>,
    ) -> Genesis {
        Genesis {
            timestamp:   SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            prevhash:    "0000000000000000000000000000".to_owned(),
            state_alloc: vec![StateAlloc {
                code,
                address,
                storage,
                balance,
            }],
        }
    }
}
