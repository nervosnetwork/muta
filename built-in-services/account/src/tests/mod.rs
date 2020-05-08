use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use cita_trie::MemoryDB;
use rand::rngs::OsRng;

use async_trait::async_trait;
use common_crypto::{
    Crypto, PrivateKey, PublicKey, Secp256k1, Secp256k1PrivateKey, Signature, ToPublicKey,
};

use framework::binding::sdk::{DefalutServiceSDK, DefaultChainQuerier};
use framework::binding::state::{GeneralServiceState, MPTTrie};
use protocol::traits::{NoopDispatcher, Storage, Witness};
use protocol::types::{
    Address, Block, Hash, Hex, Proof, Receipt, ServiceContext, ServiceContextParams,
    SignedTransaction,
};

use protocol::{types::Bytes, ProtocolResult};

use crate::types::{
    GenerateAccountPayload, GetAccountPayload, PayloadAccount, VerifyPayload, WitnessAdapter,
    ACCOUNT_TYPE_MULTI_SIG,
};
use crate::AccountService;

#[test]
fn test_generate() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller);
    let mut service = new_account_service();

    let priv_key1 = Secp256k1PrivateKey::generate(&mut OsRng);
    let pub_key1 = priv_key1.pub_key();
    let addr1 = Address::from_pubkey_bytes(pub_key1.to_bytes()).unwrap();

    let priv_key2 = Secp256k1PrivateKey::generate(&mut OsRng);
    let pub_key2 = priv_key2.pub_key();
    let addr2 = Address::from_pubkey_bytes(pub_key2.to_bytes()).unwrap();

    let priv_key3 = Secp256k1PrivateKey::generate(&mut OsRng);
    let pub_key3 = priv_key3.pub_key();
    let addr3 = Address::from_pubkey_bytes(pub_key3.to_bytes()).unwrap();

    let multi_addr = Address::from_hex("0x6fcd3e8e97da273711ccefb79abdd246c5663c7d").unwrap();

    println!(
        "{}\r\n{}\r\n{}\r\n",
        addr1.clone().as_hex(),
        addr2.as_hex(),
        addr3.as_hex()
    );

    let mut accounts = Vec::<PayloadAccount>::new();
    accounts.push(PayloadAccount {
        address: addr1.clone(),
        weight:  2,
    });

    accounts.push(PayloadAccount {
        address: addr2,
        weight:  1,
    });

    accounts.push(PayloadAccount {
        address: addr3,
        weight:  1,
    });

    let res = service.generate_account(context.clone(), GenerateAccountPayload {
        accounts,
        threshold: 3,
    });

    let addr = res.succeed_data.address.clone();
    let res_get =
        service.get_account_from_address(context.clone(), GetAccountPayload { user: addr });

    println!("{:#?}", res);
    println!("{:#?}", res_get);

    let tx_hash = Hash::from_empty();
    let sig1 = Secp256k1::sign_message(&tx_hash.as_bytes(), &priv_key1.to_bytes()).unwrap();
    let mut input_sig: String = "0x".to_string() + hex::encode(sig1.to_bytes()).as_str();
    let sig_data1 = Hex::from_string(input_sig).unwrap();
    let pk_1 =
        Hex::from_string("0x".to_string() + hex::encode(pub_key1.to_bytes()).as_str()).unwrap();

    let sig2 = Secp256k1::sign_message(&tx_hash.as_bytes(), &priv_key2.to_bytes()).unwrap();
    input_sig = "0x".to_string() + hex::encode(sig2.to_bytes()).as_str();
    let sig_data2 = Hex::from_string(input_sig).unwrap();
    let pk_2 =
        Hex::from_string("0x".to_string() + hex::encode(pub_key2.to_bytes()).as_str()).unwrap();

    let sig3 = Secp256k1::sign_message(&tx_hash.as_bytes(), &priv_key3.to_bytes()).unwrap();
    input_sig = "0x".to_string() + hex::encode(sig3.to_bytes()).as_str();
    let sig_data3 = Hex::from_string(input_sig).unwrap();
    let pk_3 =
        Hex::from_string("0x".to_string() + hex::encode(pub_key3.to_bytes()).as_str()).unwrap();

    // verify single sig, sig1, signature_type error, expected not verified
    let mut wit_single = WitnessAdapter {
        pubkeys:        vec![pk_1.clone()],
        signatures:     vec![sig_data1.clone()],
        signature_type: ACCOUNT_TYPE_MULTI_SIG,
        sender:         addr1,
    };

    let mut wit_str = serde_json::to_string(&wit_single).unwrap();

    let mut res_single = service.verify_signature(context.clone(), VerifyPayload {
        tx_hash: tx_hash.clone(),
        witness: wit_str.clone(),
    });
    assert_eq!(res_single.is_error(), true);
    println!("single sig1, expected not verified\r\n {:#?}", res_single);

    // verify single sig, sig1,  expected  verified
    wit_single =
        WitnessAdapter::from_single_sig_hex(pk_1.as_string(), sig_data1.as_string()).unwrap();
    wit_str = wit_single.as_string().unwrap();

    res_single = service.verify_signature(context.clone(), VerifyPayload {
        tx_hash: tx_hash.clone(),
        witness: wit_str.clone(),
    });
    assert_eq!(res_single.is_error(), false);
    println!("single sig1, expect verified\r\n {:#?}", res_single);

    // verify single sig, sig1,  expected not verified
    wit_single =
        WitnessAdapter::from_single_sig_hex(pk_2.as_string(), sig_data1.as_string()).unwrap();
    wit_str = wit_single.as_string().unwrap();

    res_single = service.verify_signature(context.clone(), VerifyPayload {
        tx_hash: tx_hash.clone(),
        witness: wit_str,
    });
    assert_eq!(res_single.is_error(), true);
    println!("single sig1-pk2, expect not verified\r\n {:#?}", res_single);

    // verify multiSig, sig1+sig2, expected verified
    let wit1 = WitnessAdapter::from_multi_sig_hex(
        multi_addr.clone(),
        vec![pk_1.as_string(), pk_2.as_string()],
        vec![sig_data1.as_string(), sig_data2.as_string()],
    )
    .unwrap();

    let wit1_str = wit1.as_string().unwrap();

    let res_multi_1 = service.verify_signature(context.clone(), VerifyPayload {
        tx_hash: tx_hash.clone(),
        witness: wit1_str,
    });
    assert_eq!(res_multi_1.is_error(), false);
    println!("multisig sig1+sig2, expect verified,\r\n{:#?}", res_multi_1);

    // verify multiSig, sig2+sig3, expected not verified
    let wit2 = WitnessAdapter {
        pubkeys:        vec![pk_2.clone(), pk_3.clone()],
        signatures:     vec![sig_data2.clone(), sig_data3.clone()],
        signature_type: ACCOUNT_TYPE_MULTI_SIG,
        sender:         multi_addr.clone(),
    };

    let wit2_str = serde_json::to_string(&wit2).unwrap();

    let res_multi_2 = service.verify_signature(context.clone(), VerifyPayload {
        tx_hash: tx_hash.clone(),
        witness: wit2_str,
    });
    assert_eq!(res_multi_2.is_error(), true);
    println!("{:#?}", res_multi_2);

    // verify multiSig, sig1+ sig2 + sig3, expected verified
    let wit3 = WitnessAdapter {
        pubkeys:        vec![pk_1.clone(), pk_2, pk_3],
        signatures:     vec![sig_data1.clone(), sig_data2, sig_data3],
        signature_type: ACCOUNT_TYPE_MULTI_SIG,
        sender:         multi_addr.clone(),
    };

    let wit3_str = serde_json::to_string(&wit3).unwrap();

    let res_multi_3 = service.verify_signature(context.clone(), VerifyPayload {
        tx_hash: tx_hash.clone(),
        witness: wit3_str,
    });
    assert_eq!(res_multi_3.is_error(), false);
    println!("{:#?}", res_multi_3);

    // verify multiSig, sig1, expected not verified
    let wit4 = WitnessAdapter {
        pubkeys:        vec![pk_1],
        signatures:     vec![sig_data1],
        signature_type: ACCOUNT_TYPE_MULTI_SIG,
        sender:         multi_addr,
    };

    let wit4_str = serde_json::to_string(&wit4).unwrap();

    let res_multi_4 = service.verify_signature(context, VerifyPayload {
        tx_hash,
        witness: wit4_str,
    });
    assert_eq!(res_multi_4.is_error(), true);
    println!("{:#?}", res_multi_4);
}

fn new_account_service() -> AccountService<
    DefalutServiceSDK<
        GeneralServiceState<MemoryDB>,
        DefaultChainQuerier<MockStorage>,
        NoopDispatcher,
    >,
> {
    let chain_db = DefaultChainQuerier::new(Arc::new(MockStorage {}));
    let trie = MPTTrie::new(Arc::new(MemoryDB::new(false)));
    let state = GeneralServiceState::new(trie);

    let sdk = DefalutServiceSDK::new(
        Rc::new(RefCell::new(state)),
        Rc::new(chain_db),
        NoopDispatcher {},
    );
    AccountService::new(sdk)
}

fn mock_context(cycles_limit: u64, caller: Address) -> ServiceContext {
    let params = ServiceContextParams {
        tx_hash: Some(Hash::from_empty()),
        nonce: None,
        cycles_limit,
        cycles_price: 1,
        cycles_used: Rc::new(RefCell::new(0)),
        caller,
        height: 1,
        timestamp: 0,
        service_name: "service_name".to_owned(),
        service_method: "service_method".to_owned(),
        service_payload: "service_payload".to_owned(),
        extra: None,
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

    async fn load_overlord_wal(&self) -> ProtocolResult<Bytes> {
        unimplemented!()
    }
}
