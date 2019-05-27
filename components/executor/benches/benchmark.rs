#![feature(test)]

extern crate test;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use test::Bencher;

use core_context::Context;
use core_runtime::executor::Executor;
use core_runtime::ExecutionContext;
use core_types::{Address, Hash, SignedTransaction, Transaction, UnverifiedTransaction, U256};

const ERC20_CODE: &str =
    "606060405234620000005760405162001617380380620016178339810160405280805190602001909190805182\
     01919060200180519060200190919080518201919050505b83600560003373ffffffffffffffffffffffffffff\
     ffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000208190\
     555083600381905550826000908051906020019082805460018160011615610100020316600290049060005260\
     2060002090601f016020900481019282601f10620000dd57805160ff19168380011785556200010e565b828001\
     600101855582156200010e579182015b828111156200010d578251825591602001919060010190620000f0565b\
     5b5090506200013691905b808211156200013257600081600090555060010162000118565b5090565b50508060\
     019080519060200190828054600181600116156101000203166002900490600052602060002090601f01602090\
     0481019282601f106200018657805160ff1916838001178555620001b7565b82800160010185558215620001b7\
     579182015b82811115620001b657825182559160200191906001019062000199565b5b509050620001df91905b\
     80821115620001db576000816000905550600101620001c1565b5090565b505081600260006101000a81548160\
     ff021916908360ff16021790555033600460006101000a81548173ffffffffffffffffffffffffffffffffffff\
     ffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505b505050505b6113c48062\
     0002536000396000f300606060405236156100ce576000357c0100000000000000000000000000000000000000\
     000000000000000000900463ffffffff16806306fdde03146100d7578063095ea7b31461016d57806318160ddd\
     146101c157806323b872dd146101e4578063313ce5671461025757806342966c68146102805780636623fc4614\
     6102b557806370a08231146102ea5780638da5cb5b1461033157806395d89b4114610380578063a9059cbb1461\
     0416578063cd4217c114610452578063d7a78db814610499578063dd62ed3e146104ce575b6100d55b5b565b00\
     5b34610000576100e4610534565b60405180806020018281038252838181518152602001915080519060200190\
     80838360008314610133575b805182526020831115610133576020820191506020810190506020830392506101\
     0f565b505050905090810190601f16801561015f5780820380516001836020036101000a031916815260200191\
     505b509250505060405180910390f35b34610000576101a7600480803573ffffffffffffffffffffffffffffff\
     ffffffffff169060200190919080359060200190919050506105d2565b60405180821515151581526020019150\
     5060405180910390f35b34610000576101ce61066f565b6040518082815260200191505060405180910390f35b\
     346100005761023d600480803573ffffffffffffffffffffffffffffffffffffffff1690602001909190803573\
     ffffffffffffffffffffffffffffffffffffffff16906020019091908035906020019091905050610675565b60\
     4051808215151515815260200191505060405180910390f35b3461000057610264610a9b565b604051808260ff\
     1660ff16815260200191505060405180910390f35b346100005761029b6004808035906020019091905050610a\
     ae565b604051808215151515815260200191505060405180910390f35b34610000576102d06004808035906020\
     019091905050610c01565b604051808215151515815260200191505060405180910390f35b346100005761031b\
     600480803573ffffffffffffffffffffffffffffffffffffffff16906020019091905050610dce565b60405180\
     82815260200191505060405180910390f35b346100005761033e610de6565b604051808273ffffffffffffffff\
     ffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019150506040\
     5180910390f35b346100005761038d610e0c565b60405180806020018281038252838181518152602001915080\
     519060200190808383600083146103dc575b8051825260208311156103dc576020820191506020810190506020\
     830392506103b8565b505050905090810190601f1680156104085780820380516001836020036101000a031916\
     815260200191505b509250505060405180910390f35b3461000057610450600480803573ffffffffffffffffff\
     ffffffffffffffffffffff16906020019091908035906020019091905050610eaa565b005b3461000057610483\
     600480803573ffffffffffffffffffffffffffffffffffffffff16906020019091905050611138565b60405180\
     82815260200191505060405180910390f35b34610000576104b46004808035906020019091905050611150565b\
     604051808215151515815260200191505060405180910390f35b346100005761051e600480803573ffffffffff\
     ffffffffffffffffffffffffffffff1690602001909190803573ffffffffffffffffffffffffffffffffffffff\
     ff1690602001909190505061131d565b6040518082815260200191505060405180910390f35b60008054600181\
     600116156101000203166002900480601f01602080910402602001604051908101604052809291908181526020\
     01828054600181600116156101000203166002900480156105ca5780601f1061059f5761010080835404028352\
     91602001916105ca565b820191906000526020600020905b8154815290600101906020018083116105ad578290\
     03601f168201915b505050505081565b60006000821115156105e357610000565b81600760003373ffffffffff\
     ffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081\
     5260200160002060008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffff\
     ffffffffffffffff16815260200190815260200160002081905550600190505b92915050565b60035481565b60\
     0060008373ffffffffffffffffffffffffffffffffffffffff16141561069b57610000565b6000821115156106\
     aa57610000565b81600560008673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffff\
     ffffffffffffffffffffff1681526020019081526020016000205410156106f657610000565b600560008473ff\
     ffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260\
     20019081526020016000205482600560008673ffffffffffffffffffffffffffffffffffffffff1673ffffffff\
     ffffffffffffffffffffffffffffffff1681526020019081526020016000205401101561078357610000565b60\
     0760008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffff\
     ffff16815260200190815260200160002060003373ffffffffffffffffffffffffffffffffffffffff1673ffff\
     ffffffffffffffffffffffffffffffffffff1681526020019081526020016000205482111561080c5761000056\
     5b610855600560008673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffff\
     ffffffffffffff1681526020019081526020016000205483611342565b600560008673ffffffffffffffffffff\
     ffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160\
     0020819055506108e1600560008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffff\
     ffffffffffffffffffffffff168152602001908152602001600020548361135c565b600560008573ffffffffff\
     ffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081\
     52602001600020819055506109aa600760008673ffffffffffffffffffffffffffffffffffffffff1673ffffff\
     ffffffffffffffffffffffffffffffffff16815260200190815260200160002060003373ffffffffffffffffff\
     ffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001\
     6000205483611342565b600760008673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffff\
     ffffffffffffffffffffffffff16815260200190815260200160002060003373ffffffffffffffffffffffffff\
     ffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002081\
     9055508273ffffffffffffffffffffffffffffffffffffffff168473ffffffffffffffffffffffffffffffffff\
     ffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef8460405180828152\
     60200191505060405180910390a3600190505b9392505050565b600260009054906101000a900460ff1681565b\
     600081600560003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffff\
     ffffffffffff168152602001908152602001600020541015610afc57610000565b600082111515610b0b576100\
     00565b610b54600560003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffff\
     ffffffffffffffffff1681526020019081526020016000205483611342565b600560003373ffffffffffffffff\
     ffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020\
     0160002081905550610ba360035483611342565b6003819055503373ffffffffffffffffffffffffffffffffff\
     ffffff167fcc16f5dbb4873280815c1ee09dbd06736cffcc184412cf7a71a0fdb75d397ca58360405180828152\
     60200191505060405180910390a2600190505b919050565b600081600660003373ffffffffffffffffffffffff\
     ffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001600020\
     541015610c4f57610000565b600082111515610c5e57610000565b610ca7600660003373ffffffffffffffffff\
     ffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001\
     6000205483611342565b600660003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffff\
     ffffffffffffffffffffffffff16815260200190815260200160002081905550610d33600560003373ffffffff\
     ffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190\
     8152602001600020548361135c565b600560003373ffffffffffffffffffffffffffffffffffffffff1673ffff\
     ffffffffffffffffffffffffffffffffffff168152602001908152602001600020819055503373ffffffffffff\
     ffffffffffffffffffffffffffff167f2cfce4af01bcb9d6cf6c84ee1b7c491100b8695368264146a94d71e10a\
     63083f836040518082815260200191505060405180910390a2600190505b919050565b60056020528060005260\
     406000206000915090505481565b600460009054906101000a900473ffffffffffffffffffffffffffffffffff\
     ffffff1681565b60018054600181600116156101000203166002900480601f0160208091040260200160405190\
     81016040528092919081815260200182805460018160011615610100020316600290048015610ea25780601f10\
     610e7757610100808354040283529160200191610ea2565b820191906000526020600020905b81548152906001\
     0190602001808311610e8557829003601f168201915b505050505081565b60008273ffffffffffffffffffffff\
     ffffffffffffffffff161415610ece57610000565b600081111515610edd57610000565b80600560003373ffff\
     ffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020\
     01908152602001600020541015610f2957610000565b600560008373ffffffffffffffffffffffffffffffffff\
     ffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000205481600560\
     008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff\
     16815260200190815260200160002054011015610fb657610000565b610fff600560003373ffffffffffffffff\
     ffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020\
     016000205482611342565b600560003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffff\
     ffffffffffffffffffffffffffff1681526020019081526020016000208190555061108b600560008473ffffff\
     ffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001\
     908152602001600020548261135c565b600560008473ffffffffffffffffffffffffffffffffffffffff1673ff\
     ffffffffffffffffffffffffffffffffffffff168152602001908152602001600020819055508173ffffffffff\
     ffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167fddf252ad1b\
     e2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef836040518082815260200191505060405180\
     910390a35b5050565b60066020528060005260406000206000915090505481565b600081600560003373ffffff\
     ffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001\
     90815260200160002054101561119e57610000565b6000821115156111ad57610000565b6111f6600560003373\
     ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152\
     6020019081526020016000205483611342565b600560003373ffffffffffffffffffffffffffffffffffffffff\
     1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000208190555061128260\
     0660003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffff\
     ffff168152602001908152602001600020548361135c565b600660003373ffffffffffffffffffffffffffffff\
     ffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001600020819055\
     503373ffffffffffffffffffffffffffffffffffffffff167ff97a274face0b5517365ad396b1fdba6f68bd313\
     5ef603e44272adba3af5a1e0836040518082815260200191505060405180910390a2600190505b919050565b60\
     07602052816000526040600020602052806000526040600020600091509150505481565b600061135083831115\
     611388565b81830390505b92915050565b60006000828401905061137d8482101580156113785750838210155b\
     611388565b8091505b5092915050565b80151561139457610000565b5b505600a165627a7a72305820409669e0\
     0e8d4fc152b0714293e1b1c74ca1dbd321f5137c5b5dc51d29f341460029000000000000000000000000000000\
     0000000000000000000fffffffffffff6400000000000000000000000000000000000000000000000000000000\
     000000800000000000000000000000000000000000000000000000000000000000000012000000000000000000\
     00000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000\
     00000000000000000003424e420000000000000000000000000000000000000000000000000000000000000000\
     0000000000000000000000000000000000000000000000000000000003424e4200000000000000000000000000\
     00000000000000000000000000000000";

#[bench]
fn bench_new_contract(bench: &mut Bencher) {
    common_logger::init(common_logger::Flag::Test);
    let db = Arc::new(cita_vm::state::MemoryDB::new(false));
    let block_data_provider: Arc<cita_vm::BlockDataProvider> =
        Arc::new(cita_vm::BlockDataProviderMock::default());

    let genesis = core_types::Genesis {
        timestamp:   0,
        prevhash:    String::from(
            "44915be5b6c20b0678cf05fcddbbaa832e25d7e6ac538784cd5c24de00d47472",
        ),
        state_alloc: vec![
            core_types::StateAlloc {
                address: String::from("1000000000000000000000000000000000000000"),
                code:    String::from(""),
                storage: HashMap::new(),
                balance: String::from("400000000000000000"),
            },
            core_types::StateAlloc {
                address: String::from("1000000000000000000000000000000000000001"),
                code:    String::from(""),
                storage: HashMap::new(),
                balance: String::from("400000000000000000"),
            },
        ],
    };
    let (evm_executor, root_hash) = components_executor::evm::EVMExecutor::from_genesis(
        &genesis,
        Arc::<cita_trie::db::MemoryDB>::clone(&db),
        Arc::<(dyn cita_vm::BlockDataProvider + 'static)>::clone(&block_data_provider),
    )
    .unwrap();
    let mut execution_context = ExecutionContext {
        state_root:  root_hash.clone(),
        proposer:    Address::from_hex("1000000000000000000000000000000000000000").unwrap(),
        height:      1,
        quota_limit: 100_000_000_000,
        timestamp:   0,
    };

    let mut txs = vec![];
    for _ in 0..10000 {
        let tx = SignedTransaction {
            untx:   UnverifiedTransaction {
                transaction: Transaction {
                    to:                None,
                    nonce:             String::new(),
                    quota:             10_000_000,
                    valid_until_block: 100,
                    data:              hex::decode(ERC20_CODE).unwrap(),
                    value:             vec![],
                    chain_id:          vec![],
                },
                signature:   vec![],
            },
            hash:   Hash::digest(&[]),
            sender: Address::from_hex("1000000000000000000000000000000000000000").unwrap(),
        };
        txs.push(tx);
    }

    bench.iter(|| {
        let tic = SystemTime::now();
        let r = evm_executor
            .exec(Context::new(), &execution_context, &txs)
            .unwrap();
        execution_context.state_root = r.state_root;
        println!(
            "10000 tx: Executing tx: {:?}",
            SystemTime::now().duration_since(tic).unwrap()
        );
    })
}

#[bench]
fn bench_erc20(bench: &mut Bencher) {
    common_logger::init(common_logger::Flag::Test);
    let db = Arc::new(cita_vm::state::MemoryDB::new(false));
    let block_data_provider: Arc<cita_vm::BlockDataProvider> =
        Arc::new(cita_vm::BlockDataProviderMock::default());

    let genesis = core_types::Genesis {
        timestamp:   0,
        prevhash:    String::from(
            "44915be5b6c20b0678cf05fcddbbaa832e25d7e6ac538784cd5c24de00d47472",
        ),
        state_alloc: vec![
            core_types::StateAlloc {
                address: String::from("1000000000000000000000000000000000000000"),
                code:    String::from(""),
                storage: HashMap::new(),
                balance: String::from("400000000000000000"),
            },
            core_types::StateAlloc {
                address: String::from("1000000000000000000000000000000000000001"),
                code:    String::from(""),
                storage: HashMap::new(),
                balance: String::from("400000000000000000"),
            },
        ],
    };
    let (evm_executor, root_hash) = components_executor::evm::EVMExecutor::from_genesis(
        &genesis,
        Arc::<cita_trie::db::MemoryDB>::clone(&db),
        Arc::<(dyn cita_vm::BlockDataProvider + 'static)>::clone(&block_data_provider),
    )
    .unwrap();
    let mut execution_context = ExecutionContext {
        state_root:  root_hash.clone(),
        proposer:    Address::from_hex("1000000000000000000000000000000000000000").unwrap(),
        height:      1,
        quota_limit: 100_000_000_000,
        timestamp:   0,
    };

    // Deploy contract
    let tx = SignedTransaction {
        untx:   UnverifiedTransaction {
            transaction: Transaction {
                to:                None,
                nonce:             String::new(),
                quota:             10_000_000,
                valid_until_block: 100,
                data:              hex::decode(ERC20_CODE).unwrap(),
                value:             vec![],
                chain_id:          vec![],
            },
            signature:   vec![],
        },
        hash:   Hash::digest(&[]),
        sender: Address::from_hex("1000000000000000000000000000000000000000").unwrap(),
    };
    let r = evm_executor
        .exec(Context::new(), &execution_context, &[tx])
        .unwrap();
    execution_context.state_root = r.state_root;
    let contract_address = r.receipts[0].contract_address.clone().unwrap();

    let mut txs = vec![];
    for _ in 0..10000 {
        let tx = SignedTransaction {
            untx:   UnverifiedTransaction {
                transaction: Transaction {
                    to:                Some(contract_address.clone()),
                    nonce:             String::new(),
                    quota:             10_000_000,
                    valid_until_block: 10,
                    data:              hex::decode("a9059cbb0000000000000000000000001000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000a").unwrap(),
                    value:             vec![],
                    chain_id:          vec![0x00, 0x01, 0x02, 0x03],
                },
                signature:   vec![],
            },
            hash:   Hash::digest(&[]),
            sender: Address::from_hex("1000000000000000000000000000000000000000").unwrap(),
        };
        txs.push(tx);
    }

    bench.iter(|| {
        let tic = SystemTime::now();
        let r = evm_executor
            .exec(Context::new(), &execution_context, &txs)
            .unwrap();
        println!(
            "10000 tx: Executing tx: {:?}",
            SystemTime::now().duration_since(tic).unwrap()
        );
        execution_context.state_root = r.state_root;

        // Get the balance of address 0x1000...0001;
        let r = evm_executor
            .readonly(
                Context::new(),
                &execution_context,
                &contract_address,
                &Address::from_hex("1000000000000000000000000000000000000000").unwrap(),
                &hex::decode(
                    "70a082310000000000000000000000001000000000000000000000000000000000000001",
                )
                .unwrap()[..],
            )
            .unwrap();
        let mut buf = [0u8; 32];
        buf.copy_from_slice(&r.data.unwrap());
        // Ensure the transfer is success
        assert_eq!(U256::from_be_bytes(&buf).0[0] % 100_000, 0);
    })
}
