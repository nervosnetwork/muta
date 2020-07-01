use crate::types::{
    AddAccountPayload, GenerateMultiSigAccountPayload, GetMultiSigAccountPayload,
    MultiSigPermission, RemoveAccountPayload, SetAccountWeightPayload, SetThresholdPayload,
    UpdateAccountPayload,
};

use super::*;

#[test]
fn test_generate_multi_signature() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let caller = Address::from_hex("0x755cdba6ae4f479f7164792b318b2a06c759833b").unwrap();
    let context = mock_context(cycles_limit, caller);

    let mut service = new_multi_signature_service();
    let owner = Address::from_pubkey_bytes(gen_one_keypair().1).unwrap();

    // test permission accounts above the max value
    let accounts = gen_keypairs(17)
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address =
        service.generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:            owner.clone(),
            addr_with_weight: accounts,
            threshold:        12,
            memo:             String::new(),
        });
    assert!(multi_sig_address.is_error());

    // test the threshold larger than the sum of weights
    let accounts = gen_keypairs(4)
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address =
        service.generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:            owner.clone(),
            addr_with_weight: accounts,
            threshold:        12,
            memo:             String::new(),
        });
    assert!(multi_sig_address.is_error());

    // test generate a multi-signature address
    let accounts = gen_keypairs(4)
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address =
        service.generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:            owner.clone(),
            addr_with_weight: accounts.clone(),
            threshold:        3,
            memo:             String::new(),
        });
    assert!(!multi_sig_address.is_error());

    // test get permission by multi-signature address
    let addr = multi_sig_address.succeed_data.address;
    let permission = service.get_account_from_address(context, GetMultiSigAccountPayload {
        multi_sig_address: addr,
    });
    assert!(!permission.is_error());
    assert_eq!(permission.succeed_data.permission, MultiSigPermission {
        owner,
        accounts: to_accounts_list(accounts),
        threshold: 3,
        memo: String::new(),
    });
}

#[test]
fn test_set_threshold() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let mut service = new_multi_signature_service();
    let owner = gen_one_keypair();
    let owner_address = Address::from_pubkey_bytes(owner.1).unwrap();
    let context = mock_context(cycles_limit, owner_address.clone());
    let keypairs = gen_keypairs(4);
    let account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:            owner_address,
            addr_with_weight: account_pubkeys,
            threshold:        3,
            memo:             String::new(),
        })
        .succeed_data
        .address;

    // test new threshold above sum of the weights
    let res = service.set_threshold(context.clone(), SetThresholdPayload {
        multi_sig_address: multi_sig_address.clone(),
        new_threshold:     5,
    });
    assert_eq!(
        res.error_message,
        "new threshold larger the sum of the weights".to_owned()
    );

    // test set new threshold success
    let res = service.set_threshold(context, SetThresholdPayload {
        multi_sig_address,
        new_threshold: 2,
    });
    assert_eq!(res.error_message, "".to_owned());
}

#[test]
fn test_add_account() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let mut service = new_multi_signature_service();
    let owner = gen_one_keypair();
    let owner_address = Address::from_pubkey_bytes(owner.1).unwrap();
    let context = mock_context(cycles_limit, owner_address.clone());
    let keypairs = gen_keypairs(15);
    let mut account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:            owner_address.clone(),
            addr_with_weight: account_pubkeys.clone(),
            threshold:        3,
            memo:             String::new(),
        })
        .succeed_data
        .address;

    // test add new account success
    let new_keypair = gen_one_keypair();
    account_pubkeys.push(to_multi_sig_account(new_keypair.1.clone()));
    let res = service.add_account(context.clone(), AddAccountPayload {
        multi_sig_address: multi_sig_address.clone(),
        new_account:       to_multi_sig_account(new_keypair.1).into_signle_account(),
    });
    assert_eq!(res.error_message, "".to_owned());

    // test add new account success above max count value
    let new_keypair = gen_one_keypair();
    let res = service.add_account(context.clone(), AddAccountPayload {
        multi_sig_address: multi_sig_address.clone(),
        new_account:       to_multi_sig_account(new_keypair.1).into_signle_account(),
    });
    assert_eq!(
        res.error_message,
        "the account count reach max value".to_owned()
    );

    // test get permission after add a new account
    let permission =
        service.get_account_from_address(context, GetMultiSigAccountPayload { multi_sig_address });
    assert_eq!(permission.succeed_data.permission, MultiSigPermission {
        owner:     owner_address,
        accounts:  to_accounts_list(account_pubkeys),
        threshold: 3,
        memo:      String::new(),
    });
}

#[test]
fn test_update_account() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let mut service = new_multi_signature_service();
    let owner = gen_one_keypair();
    let owner_address = Address::from_pubkey_bytes(owner.1).unwrap();
    let context = mock_context(cycles_limit, owner_address.clone());
    let keypairs = gen_keypairs(4);
    let account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:            owner_address.clone(),
            addr_with_weight: account_pubkeys.clone(),
            threshold:        4,
            memo:             String::new(),
        })
        .succeed_data
        .address;

    let new_owner = gen_one_keypair();
    let new_owner_address = Address::from_pubkey_bytes(new_owner.1.clone()).unwrap();
    let context = mock_context(cycles_limit, owner_address.clone());
    let account_pubkeys = vec![AddressWithWeight {
        address: multi_sig_address.clone(),
        weight:  1u8,
    }];
    let res = service.update_account(context.clone(), UpdateAccountPayload {
        account_address:  multi_sig_address.clone(),
        new_account_info: GenerateMultiSigAccountPayload {
            owner:            new_owner_address.clone(),
            addr_with_weight: account_pubkeys,
            threshold:        1,
            memo:             String::new(),
        },
    });
    assert!(res.is_error());

    let keypairs = gen_keypairs(4);
    let account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let res = service.update_account(context, UpdateAccountPayload {
        account_address:  multi_sig_address,
        new_account_info: GenerateMultiSigAccountPayload {
            owner:            new_owner_address,
            addr_with_weight: account_pubkeys,
            threshold:        1,
            memo:             String::new(),
        },
    });
    assert_eq!(res.is_error(), false);
}

#[test]
fn test_set_weight() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let mut service = new_multi_signature_service();
    let owner = gen_one_keypair();
    let owner_address = Address::from_pubkey_bytes(owner.1).unwrap();
    let context = mock_context(cycles_limit, owner_address.clone());
    let keypairs = gen_keypairs(4);
    let mut account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:            owner_address.clone(),
            addr_with_weight: account_pubkeys.clone(),
            threshold:        4,
            memo:             String::new(),
        })
        .succeed_data
        .address;
    let to_be_changed_address = Address::from_pubkey_bytes(keypairs[0].1.clone()).unwrap();

    // test set weight success
    let res = service.set_account_weight(context.clone(), SetAccountWeightPayload {
        multi_sig_address: multi_sig_address.clone(),
        account_address:   to_be_changed_address.clone(),
        new_weight:        2,
    });
    assert_eq!(res.error_message, "".to_owned());

    // test set an invalid weight
    let res = service.set_account_weight(context.clone(), SetAccountWeightPayload {
        multi_sig_address: multi_sig_address.clone(),
        account_address:   to_be_changed_address,
        new_weight:        0,
    });
    assert_eq!(
        res.error_message,
        "the sum of weight will below threshold".to_owned()
    );

    // test get permission after add a new account
    let permission =
        service.get_account_from_address(context, GetMultiSigAccountPayload { multi_sig_address });
    account_pubkeys[0].weight = 2;
    assert_eq!(permission.succeed_data.permission, MultiSigPermission {
        owner:     owner_address,
        accounts:  to_accounts_list(account_pubkeys),
        threshold: 4,
        memo:      String::new(),
    });
}

#[test]
fn test_remove_account() {
    let cycles_limit = 1024 * 1024 * 1024; // 1073741824
    let mut service = new_multi_signature_service();
    let owner = gen_one_keypair();
    let owner_address = Address::from_pubkey_bytes(owner.1).unwrap();
    let context = mock_context(cycles_limit, owner_address.clone());
    let keypairs = gen_keypairs(4);
    let mut account_pubkeys = keypairs
        .iter()
        .map(|pair| to_multi_sig_account(pair.1.clone()))
        .collect::<Vec<_>>();
    let multi_sig_address = service
        .generate_account(context.clone(), GenerateMultiSigAccountPayload {
            owner:            owner_address.clone(),
            addr_with_weight: account_pubkeys.clone(),
            threshold:        3,
            memo:             String::new(),
        })
        .succeed_data
        .address;
    let to_be_removed_address = Address::from_pubkey_bytes(keypairs[3].1.clone()).unwrap();

    let res = service.remove_account(context.clone(), RemoveAccountPayload {
        multi_sig_address: multi_sig_address.clone(),
        account_address:   to_be_removed_address,
    });
    account_pubkeys.pop();
    assert!(!res.is_error());

    let to_be_removed_address = Address::from_pubkey_bytes(keypairs[2].1.clone()).unwrap();
    let res = service.remove_account(context.clone(), RemoveAccountPayload {
        multi_sig_address: multi_sig_address.clone(),
        account_address:   to_be_removed_address,
    });

    assert_eq!(
        res.error_message,
        "the sum of weight will below threshold".to_owned()
    );

    let permission =
        service.get_account_from_address(context, GetMultiSigAccountPayload { multi_sig_address });
    assert_eq!(permission.succeed_data.permission, MultiSigPermission {
        owner:     owner_address,
        accounts:  to_accounts_list(account_pubkeys),
        threshold: 3,
        memo:      String::new(),
    });
}
