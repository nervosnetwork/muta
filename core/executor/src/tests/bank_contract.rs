use std::cell::RefCell;
use std::rc::Rc;

use protocol::traits::executor::contract::BankContract;
use protocol::traits::executor::InvokeContext;
use protocol::types::{Address, AssetID, Balance, ContractAddress, Hash};

use crate::native_contract::NativeBankContract;
use crate::tests::{create_state_adapter, mock_invoke_context};

#[test]
fn test_bank_contract() {
    let chain_id =
        Hash::from_hex("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let address = ContractAddress::from_hex("200000000000000000000000000000000000000000").unwrap();
    let caller = Address::from_hex("230000000000000000000000000000000000000010").unwrap();
    let state = Rc::new(RefCell::new(create_state_adapter()));
    let mut bank = NativeBankContract::new(chain_id, state);
    let fee_asset =
        AssetID::from_hex("0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap();

    let ctx = mock_invoke_context(caller, None, 0, 1_000_000, fee_asset.clone());
    let name = "Muta token".to_owned();
    let symbol = "MTT".to_owned();
    let supply = Balance::from(1e18 as u64);
    let asset = bank
        .register(
            Rc::<RefCell<InvokeContext>>::clone(&ctx),
            &address,
            name.clone(),
            symbol.clone(),
            supply.clone(),
        )
        .unwrap();
    assert_eq!(&asset.symbol, &symbol);
    assert_eq!(&asset.name, &name);
    assert_eq!(&asset.supply, &supply);
    assert_eq!(&asset.manage_contract, &address);

    // use the same address to register
    let asset2 = bank.register(
        Rc::<RefCell<InvokeContext>>::clone(&ctx),
        &address,
        name.clone(),
        symbol.clone(),
        supply.clone(),
    );
    assert_eq!(asset2.is_err(), true);

    // get asset
    let asset_get = bank
        .get_asset(Rc::<RefCell<InvokeContext>>::clone(&ctx), &asset.id)
        .unwrap();
    assert_eq!(&asset, &asset_get);
}
