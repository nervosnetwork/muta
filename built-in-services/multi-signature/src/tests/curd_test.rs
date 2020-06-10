use crate::types::{
    AddAccountPayload, ChangeOwnerPayload, GenerateMultiSigAccountPayload,
    GenerateMultiSigAccountResponse, GetMultiSigAccountPayload, GetMultiSigAccountResponse,
    MultiSigAccount, MultiSigPermission, RemoveAccountPayload, SetAccountWeightPayload,
    SetThresholdPayload, VerifySignaturePayload, Witness, MAX_PERMISSION_ACCOUNTS,
};

use super::*;

#[test]
fn test_generate_multi_signature() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());

    let mut service = new_multi_signature_service();
    let owner = Address::from_pubkey_bytes(gen_one_keypair().1).unwrap();

    // test permission accounts above the max value
    let accounts = gen_keypairs(17)
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address =
        service.generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner: owner.clone(),
            accounts,
            threshold: 12,
        });
    assert!(multi_sig_address.is_error());

    // test the threshold larger than the sum of weights
    let accounts = gen_keypairs(4)
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address =
        service.generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner: owner.clone(),
            accounts,
            threshold: 12,
        });
    assert!(multi_sig_address.is_error());

    // test generate a multi-signature address
    let accounts = gen_keypairs(4)
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address =
        service.generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:     owner.clone(),
            accounts:  accounts.clone(),
            threshold: 3,
        });
    assert!(!multi_sig_address.is_error());

    // test get permission by multi-signature address
    let addr = multi_sig_address.succeed_data.address;
    let permission = service.get_account_from_address(context.clone(), GetMultiSigAccountPayload {
        multi_sig_address: addr,
    });
    assert!(!permission.is_error());
    assert_eq!(permission.succeed_data.permission, MultiSigPermission {
        owner:     owner.clone(),
        accounts:  accounts.clone(),
        threshold: 3,
    });
}

#[test]
fn test_verify_signature() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());
    let tx_hash = context.get_tx_hash().unwrap();

    let mut service = new_multi_signature_service();
    let owner = Address::from_pubkey_bytes(gen_one_keypair().1).unwrap();
    let keypairs = gen_keypairs(4);
    let account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:     owner.clone(),
            accounts:  account_pubkeys.clone(),
            threshold: 3,
        })
        .succeed_data
        .address;

    // test multi-signature pubkey length is not equal to signature length
    let res = service.verify_signature(context.clone(), VerifySignaturePayload {
        pubkeys:    keypairs
            .iter()
            .take(2)
            .map(|pair| gen_pubkey_with_sender_bytes(pair.1.clone()))
            .collect::<Vec<_>>(),
        signatures: keypairs
            .iter()
            .map(|pair| sign(&pair.0, &tx_hash))
            .collect::<Vec<_>>(),
        sender:     multi_sig_address.clone(),
    });
    assert_eq!(
        res.error_message,
        "len of signatures and pubkeys must be equal".to_owned()
    );

    // test multi-signature below threshold
    let res = service.verify_signature(context.clone(), VerifySignaturePayload {
        pubkeys:    keypairs
            .iter()
            .take(2)
            .map(|pair| gen_pubkey_with_sender_bytes(pair.1.clone()))
            .collect::<Vec<_>>(),
        signatures: keypairs
            .iter()
            .take(2)
            .map(|pair| sign(&pair.0, &tx_hash))
            .collect::<Vec<_>>(),
        sender:     multi_sig_address.clone(),
    });
    assert_eq!(res.error_message, "multi signature not verified".to_owned());

    // test multi-signature verify error
    let res = service.verify_signature(context.clone(), VerifySignaturePayload {
        pubkeys:    keypairs
            .iter()
            .rev()
            .map(|pair| gen_pubkey_with_sender_bytes(pair.1.clone()))
            .collect::<Vec<_>>(),
        signatures: keypairs
            .iter()
            .map(|pair| sign(&pair.0, &tx_hash))
            .collect::<Vec<_>>(),
        sender:     multi_sig_address.clone(),
    });
    assert_eq!(res.error_message, "multi signature not verified".to_owned());

    // test verify multi-signature success
    let res = service.verify_signature(context.clone(), VerifySignaturePayload {
        pubkeys:    keypairs
            .iter()
            .map(|pair| gen_pubkey_with_sender_bytes(pair.1.clone()))
            .collect::<Vec<_>>(),
        signatures: keypairs
            .iter()
            .map(|pair| sign(&pair.0, &tx_hash))
            .collect::<Vec<_>>(),
        sender:     multi_sig_address.clone(),
    });
    assert_eq!(res.error_message, "".to_owned());
}

#[test]
fn test_set_threshold() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());
    let tx_hash = context.get_tx_hash().unwrap();

    let mut service = new_multi_signature_service();
    let owner = gen_one_keypair();
    let owner_address = Address::from_pubkey_bytes(owner.1.clone()).unwrap();
    let keypairs = gen_keypairs(4);
    let account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:     owner_address.clone(),
            accounts:  account_pubkeys.clone(),
            threshold: 3,
        })
        .succeed_data
        .address;

    // test new threshold above sum of the weights
    let res = service.set_threshold(context.clone(), SetThresholdPayload {
        witness:           gen_single_witness(&owner.0, &tx_hash),
        multi_sig_address: multi_sig_address.clone(),
        new_threshold:     5,
    });
    assert_eq!(
        res.error_message,
        "new threshold larger the sum of the weights".to_owned()
    );

    // test set new threshold success
    let res = service.set_threshold(context.clone(), SetThresholdPayload {
        witness:           gen_single_witness(&owner.0, &tx_hash),
        multi_sig_address: multi_sig_address.clone(),
        new_threshold:     2,
    });
    assert_eq!(res.error_message, "".to_owned());
}

#[test]
fn test_add_account() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());
    let tx_hash = context.get_tx_hash().unwrap();

    let mut service = new_multi_signature_service();
    let owner = gen_one_keypair();
    let owner_address = Address::from_pubkey_bytes(owner.1.clone()).unwrap();
    let keypairs = gen_keypairs(15);
    let mut account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:     owner_address.clone(),
            accounts:  account_pubkeys.clone(),
            threshold: 3,
        })
        .succeed_data
        .address;

    // test add new account success
    let new_keypair = gen_one_keypair();
    account_pubkeys.push(to_multi_sig_account(new_keypair.1.clone()));
    let res = service.add_account(context.clone(), AddAccountPayload {
        witness:           gen_single_witness(&owner.0, &tx_hash),
        multi_sig_address: multi_sig_address.clone(),
        new_account:       to_multi_sig_account(new_keypair.1.clone()),
    });
    assert_eq!(res.error_message, "".to_owned());

    // test add new account success above max count value
    let new_keypair = gen_one_keypair();
    let res = service.add_account(context.clone(), AddAccountPayload {
        witness:           gen_single_witness(&owner.0, &tx_hash),
        multi_sig_address: multi_sig_address.clone(),
        new_account:       to_multi_sig_account(new_keypair.1.clone()),
    });
    assert_eq!(
        res.error_message,
        "the account count reach max value".to_owned()
    );

    // test get permission after add a new account
    let permission = service.get_account_from_address(context.clone(), GetMultiSigAccountPayload {
        multi_sig_address,
    });
    assert_eq!(permission.succeed_data.permission, MultiSigPermission {
        owner:     owner_address,
        accounts:  account_pubkeys.clone(),
        threshold: 3,
    });
}

#[test]
fn test_set_weight() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());
    let tx_hash = context.get_tx_hash().unwrap();

    let mut service = new_multi_signature_service();
    let owner = gen_one_keypair();
    let owner_address = Address::from_pubkey_bytes(owner.1.clone()).unwrap();
    let keypairs = gen_keypairs(4);
    let mut account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:     owner_address.clone(),
            accounts:  account_pubkeys.clone(),
            threshold: 4,
        })
        .succeed_data
        .address;
    let to_be_changed_address = Address::from_pubkey_bytes(keypairs[0].1.clone()).unwrap();

    // test set weight success
    let res = service.set_account_weight(context.clone(), SetAccountWeightPayload {
        witness:           gen_single_witness(&owner.0, &tx_hash),
        multi_sig_address: multi_sig_address.clone(),
        account_address:   to_be_changed_address.clone(),
        new_weight:        2,
    });
    assert_eq!(res.error_message, "".to_owned());

    // test set an invalid weight
    let res = service.set_account_weight(context.clone(), SetAccountWeightPayload {
        witness:           gen_single_witness(&owner.0, &tx_hash),
        multi_sig_address: multi_sig_address.clone(),
        account_address:   to_be_changed_address,
        new_weight:        0,
    });
    assert_eq!(res.error_message, "new weight is invalid".to_owned());

    // test get permission after add a new account
    let permission = service.get_account_from_address(context.clone(), GetMultiSigAccountPayload {
        multi_sig_address,
    });
    account_pubkeys[0].weight = 2;
    assert_eq!(permission.succeed_data.permission, MultiSigPermission {
        owner:     owner_address,
        accounts:  account_pubkeys.clone(),
        threshold: 4,
    });
}

#[test]
fn test_remove_account() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller.clone());
    let tx_hash = context.get_tx_hash().unwrap();

    let mut service = new_multi_signature_service();
    let owner = gen_one_keypair();
    let owner_address = Address::from_pubkey_bytes(owner.1.clone()).unwrap();
    let keypairs = gen_keypairs(4);
    let mut account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:     owner_address.clone(),
            accounts:  account_pubkeys.clone(),
            threshold: 3,
        })
        .succeed_data
        .address;
    let to_be_removed_address = Address::from_pubkey_bytes(keypairs[3].1.clone()).unwrap();

    let res = service.remove_account(context.clone(), RemoveAccountPayload {
        witness:           gen_single_witness(&owner.0, &tx_hash),
        multi_sig_address: multi_sig_address.clone(),
        account_address:   to_be_removed_address.clone(),
    });
    account_pubkeys.pop();
    assert!(!res.is_error());

    let to_be_removed_address = Address::from_pubkey_bytes(keypairs[2].1.clone()).unwrap();
    let res = service.remove_account(context.clone(), RemoveAccountPayload {
        witness:           gen_single_witness(&owner.0, &tx_hash),
        multi_sig_address: multi_sig_address.clone(),
        account_address:   to_be_removed_address.clone(),
    });

    assert_eq!(
        res.error_message,
        "the sum of weight will below threshold after remove the account".to_owned()
    );

    let permission = service.get_account_from_address(context.clone(), GetMultiSigAccountPayload {
        multi_sig_address,
    });
    assert_eq!(permission.succeed_data.permission, MultiSigPermission {
        owner:     owner_address,
        accounts:  account_pubkeys.clone(),
        threshold: 3,
    });
}
