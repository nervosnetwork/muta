use protocol::{types::Address, Bytes, ProtocolResult};

pub trait ChainInterface {
    fn get_storage(&self, key: &Bytes) -> ProtocolResult<Bytes>;

    fn set_storage(&mut self, key: Bytes, val: Bytes) -> ProtocolResult<()>;

    fn service_call(
        &mut self,
        service: &str,
        method: &str,
        payload: &str,
        current_cycle: u64,
    ) -> ProtocolResult<(String, u64)>;

    fn contract_call(
        &mut self,
        address: Address,
        args: Bytes,
        current_cycle: u64,
    ) -> ProtocolResult<(String, u64)>;
}
