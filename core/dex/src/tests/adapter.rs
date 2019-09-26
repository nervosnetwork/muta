use protocol::types::{AssetID, UserAddress};

use crate::adapter::mock::MockDexAdapter;
use crate::adapter::DexAdapter;
use crate::types::{Config, Order, OrderBook, OrderSide, OrderState, TradingPair, UserBalance};

fn test_adapter_trait<Adapter: DexAdapter>(mut adapter: Adapter) {
    let fee_account =
        UserAddress::from_hex("0x100000000000000000000000000000000000000000").unwrap();
    let btc_holder = UserAddress::from_hex("0x100000000000000000000000000000000000000001").unwrap();
    let _usdt_holder =
        UserAddress::from_hex("0x100000000000000000000000000000000000000002").unwrap();
    let btc =
        AssetID::from_hex("0x0000000000000000000000000000000000000000000000000000000000000011")
            .unwrap();
    let usdt =
        AssetID::from_hex("0x0000000000000000000000000000000000000000000000000000000000000022")
            .unwrap();

    adapter.update_fee_account(fee_account.clone()).unwrap();
    assert_eq!(&adapter.get_fee_account().unwrap(), &fee_account);

    let pair = TradingPair {
        symbol:      "btc_usdt".to_owned(),
        base_asset:  btc.clone(),
        quote_asset: usdt.clone(),
    };
    assert_eq!(0, adapter.add_trading_pair(pair.clone()).unwrap());
    let pair2 = TradingPair {
        symbol:      "btc_usdt".to_owned(),
        base_asset:  btc.clone(),
        quote_asset: usdt.clone(),
    };
    assert_eq!(1, adapter.add_trading_pair(pair2.clone()).unwrap());
    let trading_pairs = adapter.get_trading_pairs().unwrap();
    assert_eq!(vec![pair, pair2], trading_pairs);

    let config1 = Config { fee_rate: 1 };
    adapter.new_config(config1.clone()).unwrap();
    let config2 = Config { fee_rate: 2 };
    adapter.new_config(config2.clone()).unwrap();
    let configs = adapter.get_configs().unwrap();
    assert_eq!(vec![config1, config2], configs);

    let mut balance: UserBalance = adapter.get_balance(&btc_holder).unwrap();
    assert_eq!(balance.clone(), UserBalance::default());
    balance.available.insert(btc.clone(), 0u64.into());
    adapter
        .set_balance(btc_holder.clone(), balance.clone())
        .unwrap();
    assert_eq!(balance.clone(), adapter.get_balance(&btc_holder).unwrap());

    let id = "buy1".to_owned();
    let order = Order {
        id:              id.clone(),
        nonce:           "0".to_owned(),
        trading_pair_id: 0,
        order_side:      OrderSide::Buy,
        price:           1u64.into(),
        amount:          2u64.into(),
        version:         0,
        user:            btc_holder.clone(),
        unfilled_amount: 1u64.into(),
        state:           OrderState::Canceled,
    };
    assert_eq!(adapter.set_order(order.clone()).unwrap(), id.clone());
    assert_eq!(&adapter.get_order(&id).unwrap().unwrap(), &order);

    // order book
    let fmt_order = |id: &str, state: &str, side: &str, price: u64, amount: u64| -> Order {
        Order {
            id:              id.to_owned(),
            nonce:           id.to_owned(),
            trading_pair_id: 0,
            order_side:      match side {
                "buy" => OrderSide::Buy,
                "sell" => OrderSide::Sell,
                _ => unreachable!(),
            },
            price:           price.into(),
            amount:          amount.into(),
            version:         0,
            user:            btc_holder.clone(),
            unfilled_amount: 1u64.into(),
            state:           match state {
                "pending" => OrderState::Pending,
                "canceled" => OrderState::Canceled,
                "full_filled" => OrderState::FullFilled,
                _ => unreachable!(),
            },
        }
    };
    let order_book = adapter
        .get_orderbook(order.version, order.trading_pair_id)
        .unwrap();
    assert_eq!(order_book, OrderBook::default());

    let buy_1 = fmt_order("buy-1", "pending", "buy", 1, 101);
    let buy_2 = fmt_order("buy-2", "pending", "buy", 2, 102);
    let buy_3 = fmt_order("buy-3", "pending", "buy", 3, 103);
    let sell_1 = fmt_order("sell-1", "pending", "sell", 1, 101);
    let sell_2 = fmt_order("sell-2", "pending", "sell", 2, 102);
    let sell_3 = fmt_order("sell-3", "pending", "sell", 3, 103);

    adapter.set_order(buy_1.clone()).unwrap();
    adapter.set_order(buy_2.clone()).unwrap();
    adapter.set_order(buy_3.clone()).unwrap();
    adapter.set_order(sell_1.clone()).unwrap();
    adapter.set_order(sell_2.clone()).unwrap();
    adapter.set_order(sell_3.clone()).unwrap();
    let order_book = adapter
        .get_orderbook(order.version, order.trading_pair_id)
        .unwrap();

    assert_eq!(order_book.buy_orders, vec![
        buy_3.clone(),
        buy_2.clone(),
        buy_1.clone()
    ]);
    assert_eq!(order_book.sell_orders, vec![
        sell_1.clone(),
        sell_2.clone(),
        sell_3.clone()
    ]);

    adapter
        .set_order(fmt_order("buy-1", "canceled", "buy", 1, 101))
        .unwrap();
    adapter
        .set_order(fmt_order("sell-2", "full_filled", "sell", 2, 102))
        .unwrap();
    let order_book = adapter
        .get_orderbook(order.version, order.trading_pair_id)
        .unwrap();
    assert_eq!(order_book.buy_orders, vec![buy_3.clone(), buy_2.clone()]);
    assert_eq!(order_book.sell_orders, vec![sell_1.clone(), sell_3.clone()]);
}

#[test]
fn test_mock_adapter() {
    let adapter = MockDexAdapter::default();
    test_adapter_trait(adapter);
}
