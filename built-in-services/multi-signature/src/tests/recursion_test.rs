use crate::types::{GenerateMultiSigAccountPayload, VerifySignaturePayload};

use super::*;

#[test]
fn test_recursion_verify_signature() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let mut service = new_multi_signature_service();
    let owner = Address::from_pubkey_bytes(gen_one_keypair().1).unwrap();

    let init_keypairs = gen_keypairs(4);
    let init_multi_sig_account = init_keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();

    let sender = service
        .generate_account(
            mock_context(cycles_limit, caller.clone()),
            GenerateMultiSigAccountPayload {
                owner:            owner.clone(),
                addr_with_weight: init_multi_sig_account,
                threshold:        4,
                memo:             String::new(),
            },
        )
        .succeed_data
        .address;

    let keypairs = gen_keypairs(3);
    let mut multi_sig_account = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    multi_sig_account.push(AddressWithWeight {
        address: sender,
        weight:  1u8,
    });

    let sender_new = service
        .generate_account(
            mock_context(cycles_limit, caller.clone()),
            GenerateMultiSigAccountPayload {
                owner,
                addr_with_weight: multi_sig_account,
                threshold: 4,
                memo: String::new(),
            },
        )
        .succeed_data
        .address;

    let ctx = mock_context(cycles_limit, caller);
    let tx_hash = ctx.get_tx_hash().unwrap();

    let mut pks = Vec::new();
    let mut sigs = Vec::new();

    for pair in init_keypairs.iter().chain(keypairs.iter()) {
        pks.push(pair.1.clone());
        sigs.push(sign(&pair.0, &tx_hash));
    }

    assert_eq!(pks.len(), sigs.len());

    let res = service.verify_signature(ctx, VerifySignaturePayload {
        pubkeys:    pks,
        signatures: sigs,
        sender:     sender_new,
    });
    println!("{:?}", res);
    // assert_eq!(res.is_error(), false);
}
