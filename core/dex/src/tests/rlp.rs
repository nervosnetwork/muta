use std::fmt::Debug;

use num_bigint::BigUint;
use rlp::{Decodable, Encodable};

use protocol::types::{AssetID, UserAddress};

use crate::types::*;

fn assert_rlp_works<T>(s: T)
where
    T: Encodable + Decodable + std::cmp::PartialEq + Debug,
{
    let rlp_out: Vec<u8> = rlp::encode(&s);
    let s_back = rlp::decode(&rlp_out).unwrap();
    assert_eq!(s, s_back);
}

#[test]
fn test_rlp_config() {
    assert_rlp_works(Config::default());
    assert_rlp_works(Config { fee_rate: 100 });

    assert_rlp_works(ConfigsValue::default());
    let s = vec![Config { fee_rate: 100 }, Config { fee_rate: 200 }];
    assert_rlp_works(ConfigsValue(s));
}

#[test]
fn test_rlp_user_balance() {
    assert_rlp_works(UserBalance::default());
    let asset_id =
        AssetID::from_hex("0x1000000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    let amount = BigUint::from(1u64);
    let mut user_balance = UserBalance::default();
    user_balance
        .available
        .insert(asset_id.clone(), amount.clone());
    user_balance.locked.insert(asset_id.clone(), amount.clone());
    assert_rlp_works(user_balance);
}

#[test]
fn test_rlp_order() {
    let user = UserAddress::from_hex("0x100000000000000000000000000000000000000002").unwrap();
    let order = Order {
        id: "1".to_string(),
        nonce: "12345".to_string(),
        trading_pair_id: 1,
        order_side: OrderSide::Sell,
        price: BigUint::from(1u64),
        amount: BigUint::from(2u64),
        version: 2,
        user,
        unfilled_amount: BigUint::from(3u64),
        state: OrderState::Pending,
    };
    assert_rlp_works(order);
}
