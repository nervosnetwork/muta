use protocol::traits::Context;

const TXS_ORIGINAL_KEY: &str = "txs_original";
const NETWORK_TXS: usize = 1;

pub(crate) trait TxContext {
    fn mark_network_origin_new_txs(&self) -> Self;

    fn is_network_origin_txs(&self) -> bool;
}

impl TxContext for Context {
    fn mark_network_origin_new_txs(&self) -> Self {
        self.with_value::<usize>(TXS_ORIGINAL_KEY, NETWORK_TXS)
    }

    fn is_network_origin_txs(&self) -> bool {
        self.get::<usize>(TXS_ORIGINAL_KEY) == Some(&NETWORK_TXS)
    }
}
