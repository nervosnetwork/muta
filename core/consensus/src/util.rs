use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::error::Error;

use lru_cache::LruCache;
use overlord::Crypto;
use parking_lot::RwLock;
use rlp::{Decodable, DecoderError, Encodable, Prototype, Rlp, RlpStream};

use crate::ConsensusError;
use common_crypto::{
    BlsCommonReference, BlsPrivateKey, BlsPublicKey, BlsSignature, BlsSignatureVerify, HashValue,
    PrivateKey, Signature,
};
use protocol::types::{Address, Hash, MerkleRoot, SignedTransaction};
use protocol::{Bytes, ProtocolError, ProtocolResult};

const REDUNDANCY_RATE: usize = 3;

pub struct OverlordCrypto {
    private_key: BlsPrivateKey,
    addr_pubkey: RwLock<LruCache<Bytes, BlsPublicKey>>,
    common_ref:  BlsCommonReference,
}

impl Crypto for OverlordCrypto {
    fn hash(&self, msg: Bytes) -> Bytes {
        Hash::digest(msg).as_bytes()
    }

    fn sign(&self, hash: Bytes) -> Result<Bytes, Box<dyn Error + Send>> {
        let hash = HashValue::try_from(hash.as_ref()).map_err(|_| {
            ProtocolError::from(ConsensusError::Other(
                "failed to convert hash value".to_string(),
            ))
        })?;
        let sig = self.private_key.sign_message(&hash);
        Ok(sig.to_bytes())
    }

    fn verify_signature(
        &self,
        signature: Bytes,
        hash: Bytes,
        voter: Bytes,
    ) -> Result<(), Box<dyn Error + Send>> {
        let mut map = self.addr_pubkey.write();
        let hash = HashValue::try_from(hash.as_ref()).map_err(|_| {
            ProtocolError::from(ConsensusError::Other(
                "failed to convert hash value".to_string(),
            ))
        })?;
        let pub_key = map.get_mut(&voter).ok_or_else(|| {
            ProtocolError::from(ConsensusError::Other("lose public key".to_string()))
        })?;
        let signature = BlsSignature::try_from(signature.as_ref())
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;

        signature
            .verify(&hash, &pub_key, &self.common_ref)
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;
        Ok(())
    }

    fn aggregate_signatures(
        &self,
        signatures: Vec<Bytes>,
        voters: Vec<Bytes>,
    ) -> Result<Bytes, Box<dyn Error + Send>> {
        if signatures.len() != voters.len() {
            return Err(ProtocolError::from(ConsensusError::Other(
                "signatures length does not match voters length".to_string(),
            ))
            .into());
        }

        let mut map = self.addr_pubkey.write();
        let mut sigs_pubkeys = Vec::with_capacity(signatures.len());
        for (sig, addr) in signatures.iter().zip(voters.iter()) {
            let signature = BlsSignature::try_from(sig.as_ref())
                .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;

            let pub_key = map.get_mut(addr).ok_or_else(|| {
                ProtocolError::from(ConsensusError::Other("lose public key".to_string()))
            })?;

            sigs_pubkeys.push((signature, pub_key.to_owned()));
        }

        let sig = BlsSignature::combine(sigs_pubkeys);
        Ok(sig.to_bytes())
    }

    fn verify_aggregated_signature(
        &self,
        aggregated_signature: Bytes,
        hash: Bytes,
        voters: Vec<Bytes>,
    ) -> Result<(), Box<dyn Error + Send>> {
        let mut map = self.addr_pubkey.write();
        let mut pub_keys = Vec::new();
        for addr in voters.iter() {
            let pub_key = map
                .get_mut(addr)
                .ok_or_else(|| {
                    ProtocolError::from(ConsensusError::Other("lose public key".to_string()))
                })?
                .clone();
            pub_keys.push(pub_key);
        }

        let aggregate_key = BlsPublicKey::aggregate(pub_keys);
        let aggregated_signature = BlsSignature::try_from(aggregated_signature.as_ref())
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;
        let hash = HashValue::try_from(hash.as_ref()).map_err(|_| {
            ProtocolError::from(ConsensusError::Other(
                "failed to convert hash value".to_string(),
            ))
        })?;

        aggregated_signature
            .verify(&hash, &aggregate_key, &self.common_ref)
            .map_err(|e| ProtocolError::from(ConsensusError::CryptoErr(Box::new(e))))?;
        Ok(())
    }
}

impl OverlordCrypto {
    pub fn new(
        private_key: BlsPrivateKey,
        addr_pubkey: Vec<(Bytes, BlsPublicKey)>,
        common_ref: BlsCommonReference,
    ) -> Self {
        let mut map = LruCache::new(addr_pubkey.len() * REDUNDANCY_RATE);
        map.extend(addr_pubkey.into_iter());

        OverlordCrypto {
            addr_pubkey: RwLock::new(map),
            private_key,
            common_ref,
        }
    }

    pub fn update(&self, height: u64, new_addr_pubkey: Vec<(Bytes, BlsPublicKey)>) {
        let mut map = self.addr_pubkey.write();

        if map.capacity() < new_addr_pubkey.len() * REDUNDANCY_RATE {
            map.set_capacity(new_addr_pubkey.len() * REDUNDANCY_RATE);
        }
        map.extend(new_addr_pubkey.into_iter());
        log::info!("[consensus]: crypto map {:?}", map);
    }
}

#[derive(Clone, Debug)]
pub struct ExecuteInfo {
    pub height:       u64,
    pub chain_id:     Hash,
    pub block_hash:   Hash,
    pub signed_txs:   Vec<SignedTransaction>,
    pub order_root:   MerkleRoot,
    pub cycles_price: u64,
    pub coinbase:     Address,
    pub timestamp:    u64,
    pub cycles_limit: u64,
}

impl Into<ExecWalInfo> for ExecuteInfo {
    fn into(self) -> ExecWalInfo {
        ExecWalInfo {
            height:       self.height,
            chain_id:     self.chain_id,
            block_hash:   self.block_hash,
            order_root:   self.order_root,
            cycles_price: self.cycles_price,
            coinbase:     self.coinbase,
            timestamp:    self.timestamp,
            cycles_limit: self.cycles_limit,
        }
    }
}

impl ExecuteInfo {
    pub fn from_wal_info(wal_info: ExecWalInfo, txs: Vec<SignedTransaction>) -> Self {
        ExecuteInfo {
            height:       wal_info.height,
            chain_id:     wal_info.chain_id,
            block_hash:   wal_info.block_hash,
            signed_txs:   txs,
            order_root:   wal_info.order_root,
            cycles_price: wal_info.cycles_price,
            coinbase:     wal_info.coinbase,
            timestamp:    wal_info.timestamp,
            cycles_limit: wal_info.cycles_limit,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExecWalInfo {
    pub height:       u64,
    pub chain_id:     Hash,
    pub block_hash:   Hash,
    pub order_root:   MerkleRoot,
    pub cycles_price: u64,
    pub coinbase:     Address,
    pub timestamp:    u64,
    pub cycles_limit: u64,
}

impl Encodable for ExecWalInfo {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(8)
            .append(&self.height)
            .append(&self.chain_id)
            .append(&self.block_hash)
            .append(&self.order_root)
            .append(&self.cycles_price)
            .append(&self.coinbase)
            .append(&self.timestamp)
            .append(&self.cycles_limit);
    }
}

impl Decodable for ExecWalInfo {
    fn decode(r: &Rlp) -> Result<Self, DecoderError> {
        match r.prototype()? {
            Prototype::List(8) => {
                let height: u64 = r.val_at(0)?;
                let chain_id: Hash = r.val_at(1)?;
                let block_hash: Hash = r.val_at(2)?;
                let order_root: Hash = r.val_at(3)?;
                let cycles_price: u64 = r.val_at(4)?;
                let coinbase: Address = r.val_at(5)?;
                let timestamp: u64 = r.val_at(6)?;
                let cycles_limit: u64 = r.val_at(7)?;
                Ok(ExecWalInfo {
                    height,
                    chain_id,
                    block_hash,
                    order_root,
                    cycles_price,
                    coinbase,
                    timestamp,
                    cycles_limit,
                })
            }
            _ => Err(DecoderError::RlpInconsistentLengthAndData),
        }
    }
}

#[derive(Clone, Debug)]
pub struct WalInfoQueue {
    pub inner: BTreeMap<u64, ExecWalInfo>,
}

impl Encodable for WalInfoQueue {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(1).append_list(
            &self
                .inner
                .iter()
                .map(|(_id, info)| info.clone())
                .collect::<Vec<ExecWalInfo>>(),
        );
    }
}

impl Decodable for WalInfoQueue {
    fn decode(r: &Rlp) -> Result<Self, DecoderError> {
        match r.prototype()? {
            Prototype::List(1) => {
                let tmp: Vec<ExecWalInfo> = r.list_at(0)?;
                let inner = tmp
                    .into_iter()
                    .map(|info| (info.height, info))
                    .collect::<BTreeMap<_, _>>();
                Ok(WalInfoQueue { inner })
            }
            _ => Err(DecoderError::RlpInconsistentLengthAndData),
        }
    }
}

#[allow(clippy::new_without_default)]
impl WalInfoQueue {
    pub fn new() -> Self {
        WalInfoQueue {
            inner: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, info: ExecWalInfo) {
        self.inner.insert(info.height, info);
    }

    pub fn remove_by_height(&mut self, height: u64) -> ProtocolResult<()> {
        match self.inner.remove(&height) {
            Some(_) => Ok(()),
            None => Err(ConsensusError::ExecuteErr(format!(
                "wal info queue does not contain height {}",
                height
            ))
            .into()),
        }
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bls_amcl() {
        let private_keys = vec![
            hex::decode("000000000000000000000000000000001abd6ffdb44427d9e1fcb6f84e7fe7d98f2b5b205b30a94992ec24d94bb0c970").unwrap(),
            hex::decode("00000000000000000000000000000000320b11d7c1ae66fdad1b4a75221244ae2d84903d3548c581d7d30dc135aac817").unwrap(),
            hex::decode("000000000000000000000000000000006a41e900d0426e615ca9d9393e6792baf9bda4398d5d407e59f77cb6c6f393cc").unwrap(),
            hex::decode("00000000000000000000000000000000125d81e0eb0a9c3746d868bf3b4f07760fdd430daded41d92f53b4e484ef3415").unwrap(),
        ];

        let public_keys = vec![
            hex::decode("041054fe9a65be0891094ed37fb3655e3ffb12353bc0a1b4f8673b52ad65d1ca481780cf7e988eb8dcdc05d8352f03605b0d11afb2525b3f1b55ec694509248bcfead39cbb292725d710e2a509c77ed051d1d49e15e429cf6d12b9be7c02179612").unwrap(),
            hex::decode("040c15c82ed07dc866ab7c3af3a070eb4340ac0439bf12bb49cbed5797d52707e009f7c17414777b0213b9a55c8a5c08290ce40c366d59322db418b7ff41277090bd25614174763c9fd725ede1f65f3e61ca9acdb35f59e33d556e738add14d536").unwrap(),
            hex::decode("040b3118acefdfbb11ded262a7f3c90dfca4fbc0200a92b4f6bb80210ab85e39f79458f7d47f7cb06864df0571e7591a4e0858df0b52a4c3ae19ae3adc32e1da0ec4cbdca108365ee433becdb1ccebb1b339647788dfad94ebae1cbd770fcfa4e5").unwrap(),
            hex::decode("040709f204e3ec5b8bdd9f2bb6edc9cb1704fc1e4952661ba7532ea8e37f3b159b8d41987ee6707d32bdf494e2deb00b7f049a4670a5ce1ad8e429fcacc5bbc69cb03b71a7f1d831d0b47dda5e62642d420ff0a545950cb1db19d42fe04e2c91d2").unwrap(),
        ];

        let msg = Hash::digest(Bytes::from("muta-consensus"));
        let hash = HashValue::try_from(msg.as_bytes().as_ref()).unwrap();
        let mut sigs_and_pub_keys = Vec::new();
        for i in 0..3 {
            let sig = BlsPrivateKey::try_from(private_keys[i].as_ref())
                .unwrap()
                .sign_message(&hash);
            let pub_key = BlsPublicKey::try_from(public_keys[i].as_ref()).unwrap();
            sigs_and_pub_keys.push((sig, pub_key));
        }

        let signature = BlsSignature::combine(sigs_and_pub_keys.clone());
        let aggregate_key =
            BlsPublicKey::aggregate(sigs_and_pub_keys.iter().map(|s| &s.1).collect::<Vec<_>>());

        let res = signature.verify(&hash, &aggregate_key, &"muta".into());
        println!("{:?}", res);
        assert!(res.is_ok());
    }
}
