use ethereum_types::Address;

#[derive(Default, Debug)]
pub struct ExecutorConfig {
    pub economics_model: EconomicsModel,
}

#[derive(Debug)]
pub enum EconomicsModel {
    Quota,
    Charge(ChargeConfig),
}

#[derive(Debug)]
pub struct ChargeConfig {
    pub gas_price: u64,
    /// if set coinbase, the transaction reward will be given to coinbase,
    /// otherwise it will be given to proposer
    pub coinbase: Option<Address>,
}

impl Default for EconomicsModel {
    fn default() -> Self {
        EconomicsModel::Charge(ChargeConfig {
            gas_price: 1,
            coinbase:  None,
        })
    }
}
