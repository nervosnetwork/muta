use protocol::types::UserAddress;
use protocol::ProtocolResult;

use crate::types::{Config, Order, OrderBook, TradingPair, UserBalance};

pub trait DexAdapter {
    // state
    fn get_fee_account(&self) -> ProtocolResult<UserAddress>;
    fn update_fee_account(&mut self, new_account: UserAddress) -> ProtocolResult<()>;
    fn get_trading_pairs(&self) -> ProtocolResult<Vec<TradingPair>>;
    fn add_trading_pair(&mut self, pair: TradingPair) -> ProtocolResult<u64>;
    fn new_config(&mut self, config: Config) -> ProtocolResult<u64>;
    fn get_configs(&self) -> ProtocolResult<Vec<Config>>;
    fn get_balance(&self, user: &UserAddress) -> ProtocolResult<UserBalance>;
    fn set_balance(&mut self, user: UserAddress, balances: UserBalance) -> ProtocolResult<()>;
    fn get_order(&self, id: &str) -> ProtocolResult<Option<Order>>;
    fn set_order(&mut self, order: Order) -> ProtocolResult<String>;
    fn get_orderbook(&self, version: u64, trading_pair_id: u64) -> ProtocolResult<OrderBook>;
}
