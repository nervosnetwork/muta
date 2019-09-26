use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use num_bigint::BigUint;
use protocol::types::{AssetID, UserAddress};

use crate::adapter::mock::MockDexAdapter;
use crate::types::{Config, Deal, OrderSide, OrderState, PlaceOrderArgs, TradingPair};
use crate::DexContract;

// change "1.3e5" style to "130000", used for bituint init
fn parse_e(input: &str) -> Result<String, String> {
    let s1 = input.split('e').collect::<Vec<_>>();
    if s1.len() == 1 {
        Ok(s1[0].to_owned())
    } else if s1.len() == 2 {
        let base = s1[0];
        let mut tail_zero_num = s1[1].parse::<usize>().unwrap();
        let s2 = base.split('.').collect::<Vec<_>>();
        if s2.len() == 1 {
        } else if s2.len() == 2 {
            tail_zero_num -= s2[1].len();
        } else {
            return Err("contains multiple '.'".to_string());
        }
        Ok(format!("{}{}", s2.join(""), "0".repeat(tail_zero_num)))
    } else {
        Err("contains multiple e".to_string())
    }
}

fn biguint(input: &str) -> BigUint {
    BigUint::from_str(&parse_e(input).unwrap()).unwrap()
}

#[test]
fn test_dex_basic() {
    let btc_holder = UserAddress::from_hex("0x100000000000000000000000000000000000000001").unwrap();
    let usdt_holder =
        UserAddress::from_hex("0x100000000000000000000000000000000000000002").unwrap();
    let btc =
        AssetID::from_hex("0x0000000000000000000000000000000000000000000000000000000000000011")
            .unwrap();
    let usdt =
        AssetID::from_hex("0x0000000000000000000000000000000000000000000000000000000000000022")
            .unwrap();
    let adapter = Rc::new(RefCell::new(MockDexAdapter::default()));
    let mut dex = DexContract::new(Rc::clone(&adapter));

    let fee_account =
        UserAddress::from_hex("0x100000000000000000000000000000000000000000").unwrap();
    dex.update_fee_account(fee_account.clone()).unwrap();

    // add trading pair
    let pair = TradingPair {
        symbol:      "btc_usdt".to_owned(),
        base_asset:  btc.clone(),
        quote_asset: usdt.clone(),
    };
    let pair_id = dex.add_trading_pair(pair).unwrap();
    assert_eq!(pair_id, 0);

    // add config
    let config = Config {
        fee_rate: 100_000u64,
    };
    let version = dex.new_config(config).unwrap();
    assert_eq!(version, 0);

    // deposit
    dex.deposit(
        btc_holder.clone(),
        btc.clone(),
        &BigUint::from_str(&parse_e("3e18").unwrap()).unwrap(),
    )
    .unwrap();
    dex.deposit(
        usdt_holder.clone(),
        usdt.clone(),
        &BigUint::from_str(&parse_e("20000e18").unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        adapter.borrow().balances[&usdt_holder].available[&usdt],
        biguint("20000e18")
    );
    assert_eq!(
        adapter.borrow().balances[&btc_holder].available[&btc],
        biguint("3e18")
    );

    // place order
    let mut place_order =
        |user: &UserAddress, buy: &str, nonce: &str, price: &str, amount: &str| -> String {
            dex.place_order(user, PlaceOrderArgs {
                nonce: nonce.to_owned(),
                trading_pair_id: pair_id,
                order_side: if buy == "buy" {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                },
                price: BigUint::from_str(&parse_e(price).unwrap()).unwrap(),
                amount: BigUint::from_str(&parse_e(amount).unwrap()).unwrap(),
                version,
            })
            .unwrap()
        };
    let buy_order_id = place_order(&usdt_holder, "buy", "buy-1", "10000e8", "1e8");
    let sell_order_id = place_order(&btc_holder, "sell", "sell-5", "9999e8", "2e8");
    assert_eq!(
        adapter.borrow().balances[&usdt_holder].available[&usdt],
        biguint("10000e18")
    );
    assert_eq!(
        adapter.borrow().balances[&usdt_holder].locked[&usdt],
        biguint("10000e18")
    );
    assert_eq!(
        adapter.borrow().balances[&btc_holder].available[&btc],
        biguint("1e18")
    );
    assert_eq!(
        adapter.borrow().balances[&btc_holder].locked[&btc],
        biguint("2e18")
    );

    // dbg!(&orders);
    let deals = dex.clear().unwrap();
    // println!("{}", serde_json::to_string(&deals).unwrap());
    // [{"price":"999900000000","amount":"100000000","buy_order_id":"
    // 100000000000000000000000000000000000000002-buy-1","sell_order_id":"
    // 100000000000000000000000000000000000000001-sell-5"}]
    assert_eq!(deals[0].price, biguint("999900000000"));
    assert_eq!(deals[0].amount, biguint("100000000"));

    // check state
    // dbg!(&adapter.borrow());
    // fee rate 0.1%
    assert_eq!(
        adapter.borrow().balances[&fee_account].available[&usdt],
        biguint("9.999e18")
    );
    assert_eq!(
        adapter.borrow().balances[&fee_account].available[&btc],
        biguint("0.001e18")
    );
    assert_eq!(
        adapter.borrow().balances[&usdt_holder].available[&usdt],
        biguint("10001e18")
    );
    assert_eq!(
        adapter.borrow().balances[&usdt_holder].locked[&usdt],
        biguint("0")
    );
    assert_eq!(
        adapter.borrow().balances[&usdt_holder].available[&btc],
        biguint("0.999e18")
    );
    assert_eq!(
        adapter.borrow().balances[&btc_holder].available[&btc],
        biguint("1e18")
    );
    assert_eq!(
        adapter.borrow().balances[&btc_holder].locked[&btc],
        biguint("1e18")
    );
    assert_eq!(
        adapter.borrow().balances[&btc_holder].available[&usdt],
        biguint("9989.001e18")
    );
    assert_eq!(
        adapter.borrow().orders[&buy_order_id].unfilled_amount,
        biguint("0")
    );
    assert_eq!(
        adapter.borrow().orders[&buy_order_id].state,
        OrderState::FullFilled
    );
    assert_eq!(
        adapter.borrow().orders[&sell_order_id].unfilled_amount,
        biguint("1e8")
    );
    assert_eq!(
        adapter.borrow().orders[&sell_order_id].state,
        OrderState::Pending
    );
}

#[test]
fn test_dex_clear_batch() {
    let fee_account =
        UserAddress::from_hex("0x100000000000000000000000000000000000000000").unwrap();
    let btc_holder = UserAddress::from_hex("0x100000000000000000000000000000000000000001").unwrap();
    let usdt_holder =
        UserAddress::from_hex("0x100000000000000000000000000000000000000002").unwrap();
    let btc =
        AssetID::from_hex("0x0000000000000000000000000000000000000000000000000000000000000001")
            .unwrap();
    let usdt =
        AssetID::from_hex("0x0000000000000000000000000000000000000000000000000000000000000002")
            .unwrap();
    let adapter = Rc::new(RefCell::new(MockDexAdapter::default()));
    let mut dex = DexContract::new(adapter);

    dex.update_fee_account(fee_account.clone()).unwrap();

    // add trading pair
    let pair = TradingPair {
        symbol:      "btc_usdt".to_owned(),
        base_asset:  btc.clone(),
        quote_asset: usdt.clone(),
    };
    let pair_id = dex.add_trading_pair(pair).unwrap();
    assert_eq!(pair_id, 0);

    // add config
    let config = Config {
        fee_rate: 100_000u64,
    };
    let version = dex.new_config(config).unwrap();
    assert_eq!(version, 0);

    // deposit
    dex.deposit(btc_holder.clone(), btc.clone(), &biguint("1e24"))
        .unwrap();
    dex.deposit(usdt_holder.clone(), usdt.clone(), &biguint("1e24"))
        .unwrap();

    // place order
    let mut place_order =
        |user: &UserAddress, buy: &str, nonce: &str, price: &str, amount: &str| -> String {
            dex.place_order(user, PlaceOrderArgs {
                nonce: nonce.to_owned(),
                trading_pair_id: pair_id,
                order_side: if buy == "buy" {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                },
                price: BigUint::from_str(&parse_e(price).unwrap()).unwrap(),
                amount: BigUint::from_str(&parse_e(amount).unwrap()).unwrap(),
                version,
            })
            .unwrap()
        };
    let _orders = vec![
        place_order(&usdt_holder, "buy", "buy-1", "3.80e8", "2e8"),
        place_order(&usdt_holder, "buy", "buy-2", "3.76e8", "6e8"),
        place_order(&usdt_holder, "buy", "buy-3", "3.65e8", "4e8"),
        place_order(&usdt_holder, "buy", "buy-4", "3.60e8", "7e8"),
        place_order(&usdt_holder, "buy", "buy-5", "3.54e8", "6e8"),
        place_order(&btc_holder, "sell", "sell-1", "3.52e8", "5e8"),
        place_order(&btc_holder, "sell", "sell-2", "3.57e8", "1e8"),
        place_order(&btc_holder, "sell", "sell-3", "3.60e8", "2e8"),
        place_order(&btc_holder, "sell", "sell-4", "3.65e8", "6e8"),
        place_order(&btc_holder, "sell", "sell-5", "3.70e8", "6e8"),
    ];
    // dbg!(&orders);
    let deals = dex.clear().unwrap();
    // dbg!(&deals);
    // let serialized = serde_json::to_string(&deals).unwrap();
    // println!("{}", serialized);
    let expected_deals_json = r#"
[
    {
        "price": "365000000",
        "amount": "200000000",
        "buy_order_id": "100000000000000000000000000000000000000002-buy-1",
        "sell_order_id": "100000000000000000000000000000000000000001-sell-1"
    },
    {
        "price": "365000000",
        "amount": "300000000",
        "buy_order_id": "100000000000000000000000000000000000000002-buy-2",
        "sell_order_id": "100000000000000000000000000000000000000001-sell-1"
    },
    {
        "price": "365000000",
        "amount": "100000000",
        "buy_order_id": "100000000000000000000000000000000000000002-buy-2",
        "sell_order_id": "100000000000000000000000000000000000000001-sell-2"
    },
    {
        "price": "365000000",
        "amount": "200000000",
        "buy_order_id": "100000000000000000000000000000000000000002-buy-2",
        "sell_order_id": "100000000000000000000000000000000000000001-sell-3"
    },
    {
        "price": "365000000",
        "amount": "400000000",
        "buy_order_id": "100000000000000000000000000000000000000002-buy-3",
        "sell_order_id": "100000000000000000000000000000000000000001-sell-4"
    }
]
"#;
    let expected_deals: Vec<Deal> = serde_json::from_str(expected_deals_json).unwrap();
    assert_eq!(expected_deals, deals);
}
