use bytes::Bytes;
use hasher::{Hasher, HasherKeccak};

use binding_macro::{cycles, service};
use common_crypto::{Crypto, Secp256k1};
use protocol::traits::{ExecutorParams, ServiceResponse, ServiceSDK};
use protocol::types::{Hash, ServiceContext};

use crate::types::{KeccakPayload, KeccakResponse, SigVerifyPayload, SigVerifyResponse};

#[cfg(test)]
mod tests;
pub mod types;

pub struct UtilService<SDK> {
    _sdk: SDK,
}

#[service]
impl<SDK: ServiceSDK> UtilService<SDK> {
    pub fn new(_sdk: SDK) -> Self {
        Self { _sdk }
    }

    #[cycles(100_00)]
    #[read]
    fn keccak256(
        &self,
        ctx: ServiceContext,
        payload: KeccakPayload,
    ) -> ServiceResponse<KeccakResponse> {
        let keccak = HasherKeccak::new();
        let data = hex::decode(payload.hex_str.as_string_trim0x());
        if data.is_err() {
            return ServiceResponse::<KeccakResponse>::from_error(107, "data not valid".to_owned());
        }

        let hash_res = keccak.digest(data.unwrap().as_slice());
        let response = KeccakResponse {
            result: Hash::from_bytes(Bytes::from(hash_res)).unwrap(),
        };
        ServiceResponse::<KeccakResponse>::from_succeed(response)
    }

    #[cycles(100_00)]
    #[read]
    fn verify(
        &self,
        ctx: ServiceContext,
        payload: SigVerifyPayload,
    ) -> ServiceResponse<SigVerifyResponse> {
        let data_sig = hex::decode(payload.sig.as_string_trim0x());
        if data_sig.is_err() {
            return ServiceResponse::<SigVerifyResponse>::from_error(
                108,
                "signature not valid".to_owned(),
            );
        };

        let data_pk = hex::decode(payload.pub_key.as_string_trim0x());
        if data_pk.is_err() {
            return ServiceResponse::<SigVerifyResponse>::from_error(
                109,
                "public key not valid".to_owned(),
            );
        };

        let data_hash = payload.hash.as_bytes();

        let response = SigVerifyResponse {
            is_ok: Secp256k1::verify_signature(
                data_hash.as_ref(),
                data_sig.unwrap().as_slice(),
                data_pk.unwrap().as_slice(),
            )
            .is_ok(),
        };

        ServiceResponse::<SigVerifyResponse>::from_succeed(response)
    }
}
