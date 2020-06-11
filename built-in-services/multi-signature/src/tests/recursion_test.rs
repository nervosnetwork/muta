use crate::types::{GenerateMultiSigAccountPayload, VerifySignaturePayload};

use super::*;

#[test]
fn test_recursion_verify_signature() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let mut service = new_multi_signature_service();
    let owner = Address::from_pubkey_bytes(gen_one_keypair().1).unwrap();

    let init_keypairs = gen_keypairs(4);
    let init_priv_keys = init_keypairs
        .iter()
        .map(|pair| pair.0.clone())
        .collect::<Vec<_>>();
    let init_pub_keys = init_keypairs
        .iter()
        .map(|pair| PubkeyWithSender {
            pubkey: pair.1.clone(),
            sender: None,
        })
        .collect::<Vec<_>>();
    let init_multi_sig_account = init_keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();

    let sender = service
        .generate_account(
            mock_context(cycles_limit, caller.clone()),
            GenerateMultiSigAccountPayload {
                owner:     owner.clone(),
                accounts:  init_multi_sig_account,
                threshold: 4,
                memo:      String::new(),
            },
        )
        .succeed_data
        .address;

    let keypairs = gen_keypairs(3);
    let priv_keys = keypairs
        .iter()
        .map(|pair| pair.0.clone())
        .collect::<Vec<_>>();
    let pub_keys = keypairs
        .iter()
        .map(|pair| PubkeyWithSender {
            pubkey: pair.1.clone(),
            sender: None,
        })
        .collect::<Vec<_>>();
    let mut multi_sig_account = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    multi_sig_account.push(MultiSigAccount {
        address: sender.clone(),
        weight:  1u8,
    });

    let sender_new = service
        .generate_account(
            mock_context(cycles_limit, caller.clone()),
            GenerateMultiSigAccountPayload {
                owner,
                accounts: multi_sig_account,
                threshold: 4,
                memo: String::new(),
            },
        )
        .succeed_data
        .address;

    let ctx = mock_context(cycles_limit, caller);
    let tx_hash = ctx.get_tx_hash().unwrap();
    let mut pk_with_senders = Vec::new();
    let mut sigs = Vec::new();
    pk_with_senders.push(PubkeyWithSender {
        pubkey: Bytes::from(rlp::encode_list(&init_pub_keys)),
        sender: Some(sender),
    });
    sigs.push(rlp::encode_list::<Vec<u8>, _>(&multi_sign_msg(
        &init_priv_keys,
        &tx_hash,
    )));

    for (sk, pk) in priv_keys.iter().zip(pub_keys.iter()) {
        pk_with_senders.push(pk.clone());
        sigs.push(sign(sk, &tx_hash).to_vec());
    }

    let verify_payload = VerifySignaturePayload {
        pubkeys:    Bytes::from(rlp::encode(&PubkeyWithSender {
            pubkey: Bytes::from(rlp::encode_list(&pk_with_senders)),
            sender: Some(sender_new),
        })),
        signatures: Bytes::from(rlp::encode_list::<Vec<u8>, _>(&sigs)),
    };

    let res = service.verify_signature(ctx, verify_payload);
    assert_eq!(res.is_error(), false);
}

fn multi_sign_msg(priv_keys: &[Bytes], hash: &Hash) -> Vec<Vec<u8>> {
    priv_keys
        .iter()
        .map(|key| sign(key, hash).to_vec())
        .collect::<Vec<_>>()
}
