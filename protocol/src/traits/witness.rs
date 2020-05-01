use crate::types::Address;
use crate::ProtocolResult;
use bytes::Bytes;

pub trait Witness: Sized + Send {
    fn as_bytes(&self) -> ProtocolResult<Bytes>;
    fn from_bytes(bytes: Bytes) -> ProtocolResult<Self>;
    fn as_string(&self) -> ProtocolResult<String>;
    fn from_string(s: &str) -> ProtocolResult<Self>;

    fn from_single_sig_hex(sig: String, pub_key: String) -> ProtocolResult<Self>;
    fn from_multi_sig_hex(
        sender: Address,
        sigs: Vec<String>,
        pub_keys: Vec<String>,
    ) -> ProtocolResult<Self>;
}
