use std::collections::HashMap;

use protocol::types::UserAddress;
use protocol::ProtocolResult;

use crate::adapter::traits::DexAdapter;
use crate::error::DexError;
use crate::types::{Config, Order, OrderBook, OrderSide, OrderState, TradingPair, UserBalance};

#[derive(Debug, Clone, Default)]
pub struct MockDexAdapter {
    pub fee_account:   Option<UserAddress>,
    pub configs:       Vec<Config>,
    pub trading_pairs: Vec<TradingPair>,
    pub admins:        Vec<UserAddress>,
    pub balances:      HashMap<UserAddress, UserBalance>,
    pub orders:        HashMap<String, Order>,
}

impl DexAdapter for MockDexAdapter {
    // state
    fn get_fee_account(&self) -> ProtocolResult<UserAddress> {
        self.fee_account
            .as_ref()
            .ok_or_else(|| DexError::Adapter("user address not set".to_string()).into())
            .map(|a| a.clone())
    }

    fn update_fee_account(&mut self, new_account: UserAddress) -> ProtocolResult<()> {
        self.fee_account = Some(new_account);
        Ok(())
    }

    fn get_trading_pairs(&self) -> ProtocolResult<Vec<TradingPair>> {
        Ok(self.trading_pairs.clone())
    }

    fn add_trading_pair(&mut self, pair: TradingPair) -> ProtocolResult<u64> {
        self.trading_pairs.push(pair);
        Ok((self.trading_pairs.len() - 1) as u64)
    }

    fn new_config(&mut self, config: Config) -> ProtocolResult<u64> {
        self.configs.push(config);
        Ok((self.configs.len() - 1) as u64)
    }

    fn get_configs(&self) -> ProtocolResult<Vec<Config>> {
        Ok(self.configs.clone())
    }

    fn get_balance(&self, user: &UserAddress) -> ProtocolResult<UserBalance> {
        Ok(self
            .balances
            .get(user)
            .map_or(UserBalance::default(), |b| b.clone()))
    }

    fn set_balance(&mut self, user: UserAddress, balances: UserBalance) -> ProtocolResult<()> {
        self.balances.insert(user, balances);
        Ok(())
    }

    fn get_order(&self, id: &str) -> ProtocolResult<Option<Order>> {
        Ok(self.orders.get(id).cloned())
    }

    fn set_order(&mut self, order: Order) -> ProtocolResult<String> {
        let order_id = order.id.clone();
        self.orders.insert(order_id.clone(), order);
        Ok(order_id)
    }

    fn get_orderbook(&self, version: u64, trading_pair_id: u64) -> ProtocolResult<OrderBook> {
        let mut order_book = OrderBook {
            version,
            trading_pair_id,
            ..Default::default()
        };
        for order in self.orders.iter().map(|(_, o)| o.clone()).filter(|o| {
            o.version == version
                && o.trading_pair_id == trading_pair_id
                && o.state == OrderState::Pending
        }) {
            match order.order_side {
                OrderSide::Buy => order_book.buy_orders.push(order.clone()),
                OrderSide::Sell => order_book.sell_orders.push(order.clone()),
            }
        }
        order_book
            .buy_orders
            .sort_by(|a, b| a.price.cmp(&b.price).reverse());
        order_book.sell_orders.sort_by(|a, b| a.price.cmp(&b.price));
        Ok(order_book)
    }
}
