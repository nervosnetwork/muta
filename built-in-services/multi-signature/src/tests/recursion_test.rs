use crate::types::{GenerateMultiSigAccountPayload, VerifySignaturePayload};

use super::*;

#[test]
fn test_recursion_verify_signature() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());

    let mut service = new_multi_signature_service();
    let owner = Address::from_pubkey_bytes(gen_one_keypair().1).unwrap();
    let tx_hash = context.get_tx_hash().unwrap();

    let recursion_stack = init_recursion_stack(1);
    let init_keypairs = gen_keypairs(4);
    let init_priv_keys = init_keypairs
        .iter()
        .map(|pair| pair.0.clone())
        .collect::<Vec<_>>();
    let init_pub_keys = init_keypairs
        .iter()
        .map(|pair| pair.1.clone())
        .collect::<Vec<_>>();
    let init_multi_sig_account = init_keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();

    let mut sender = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:     owner.clone(),
            accounts:  init_multi_sig_account,
            threshold: 4,
        })
        .succeed_data
        .address;
    let mut sig_bytes = multi_sign_msg(&init_priv_keys, &tx_hash);
    let mut pk_with_senders = PubkeyWithSender {
        pubkey: encode_multi_pubkeys(&init_pub_keys),
        sender: Some(sender.clone()),
    };

    for item in recursion_stack.clone().into_iter().rev() {
        let mut accounts = item.accounts.clone();
        accounts.push(MultiSigAccount {
            address: sender.clone(),
            weight:  1u8,
        });

        let mut sigs = multi_sign_msg(&item.priv_keys, &tx_hash);
        let mut pks = item
            .pub_keys
            .iter()
            .map(|pk| {
                rlp::encode(&PubkeyWithSender {
                    pubkey: pk.clone(),
                    sender: None,
                })
            })
            .collect::<Vec<_>>();
        sigs.push(rlp::encode_list::<Vec<u8>, _>(&sig_bytes));
        pks.push(rlp::encode(&pk_with_senders));

        sender = service
            .generate_account(context.clone(), GenerateMultiSigAccountPayload {
                owner: owner.clone(),
                accounts,
                threshold: 4,
            })
            .succeed_data
            .address;

        sig_bytes = sigs;
        pk_with_senders = PubkeyWithSender {
            pubkey: Bytes::from(rlp::encode_list::<Vec<u8>, _>(&pks)),
            sender: Some(sender.clone()),
        };
    }

    let verify_payload = VerifySignaturePayload {
        pubkeys:    rlp::decode_list::<Vec<u8>>(pk_with_senders.pubkey.as_ref())
            .iter()
            .map(|bytes| Bytes::from(bytes.to_vec()))
            .collect::<Vec<_>>(),
        signatures: sig_bytes
            .into_iter()
            .map(|sig| Bytes::from(sig))
            .collect::<Vec<_>>(),
        sender:     sender.clone(),
    };

    let res = service.verify_signature(context.clone(), verify_payload);
    println!("{:?}", res);
}

#[derive(Clone, Debug)]
struct RecursionStackElem {
    priv_keys: Vec<Bytes>,
    pub_keys:  Vec<Bytes>,
    accounts:  Vec<MultiSigAccount>,
}

fn init_recursion_stack(depth: usize) -> Vec<RecursionStackElem> {
    (0..depth)
        .map(|_| {
            let keypairs = gen_keypairs(3);
            RecursionStackElem {
                priv_keys: keypairs
                    .iter()
                    .map(|pair| pair.0.clone())
                    .collect::<Vec<_>>(),
                pub_keys:  keypairs
                    .iter()
                    .map(|pair| pair.1.clone())
                    .collect::<Vec<_>>(),
                accounts:  keypairs
                    .iter()
                    .map(|pair| to_multi_sig_account(pair.1.clone()))
                    .collect::<Vec<_>>(),
            }
        })
        .collect::<Vec<_>>()
}

fn multi_sign_msg(priv_keys: &Vec<Bytes>, hash: &Hash) -> Vec<Vec<u8>> {
    priv_keys
        .into_iter()
        .map(|key| sign(key, hash).to_vec())
        .collect::<Vec<_>>()
}

fn encode_multi_pubkeys(pubkeys: &Vec<Bytes>) -> Bytes {
    Bytes::from(rlp::encode_list(
        &pubkeys
            .into_iter()
            .map(|key| PubkeyWithSender {
                pubkey: key.clone(),
                sender: None,
            })
            .collect::<Vec<_>>(),
    ))
}
