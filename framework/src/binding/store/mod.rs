mod array;
mod map;
mod map_new;
mod primitive;

use bytes::Bytes;
use derive_more::{Display, From};

use protocol::fixed_codec::{FixedCodec, FixedCodecError};
use protocol::{ProtocolError, ProtocolErrorKind, ProtocolResult};

pub use array::DefaultStoreArray;
pub use map::DefaultStoreMap;
pub use primitive::{DefaultStoreBool, DefaultStoreString, DefaultStoreUint64};

pub struct FixedKeys<K: FixedCodec> {
    pub inner: Vec<K>,
}

impl<K: FixedCodec> rlp::Encodable for FixedKeys<K> {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let inner: Vec<Vec<u8>> = self
            .inner
            .iter()
            .map(|k| k.encode_fixed().expect("encode should not fail").to_vec())
            .collect();

        s.begin_list(1).append_list::<Vec<u8>, _>(&inner);
    }
}

impl<K: FixedCodec> rlp::Decodable for FixedKeys<K> {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let inner_u8: Vec<Vec<u8>> = rlp::decode_list(r.at(0)?.as_raw());

        let inner_k: Result<Vec<K>, _> = inner_u8
            .into_iter()
            .map(|v| <_>::decode_fixed(Bytes::from(v)))
            .collect();

        let inner = inner_k.map_err(|_| rlp::DecoderError::Custom("decode K from bytes fail"))?;

        Ok(FixedKeys { inner })
    }
}

impl<K: FixedCodec> FixedCodec for FixedKeys<K> {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}

pub struct FixedBuckets<K: FixedCodec + PartialEq> {
    pub keys_bucket: Vec<Bucket<K>>,
    pub bucket_lens: Vec<u32>,
    pub keys_len:    u32,
}

impl<K: FixedCodec + PartialEq> FixedBuckets<K> {
    pub fn new(buckets: Vec<Bucket<K>>) -> Self {
        let mut bucket_lens = vec![0];
        let mut keys_len = 0;

        for bkt in buckets.iter() {
            keys_len += bkt.len() as u32;
            bucket_lens.push(keys_len);
        }

        FixedBuckets {
            keys_bucket: buckets,
            bucket_lens,
            keys_len,
        }
    }

    pub fn len(&self) -> u32 {
        self.keys_len
    }

    pub fn insert(&mut self, idx: usize, key: K) {
        let bkt = self.keys_bucket.get_mut(idx).unwrap();
        bkt.push(key);

        let idx = idx + 1;
        for i in 1..=16 {
            if i >= idx {
                let tmp = self.bucket_lens[i];
                self.bucket_lens[i] = tmp + 1;
            }
        }

        self.keys_len += 1;
    }

    pub fn contains(&self, key: &K, key_bytes: &Bytes) -> bool {
        self.keys_bucket
            .get(get_bucket_index(key_bytes))
            .unwrap()
            .contains(key)
    }

    pub fn is_empty(&self) -> bool {
        self.keys_len == 0
    }

    pub fn remove_item(&mut self, key: &K, key_bytes: &Bytes) -> ProtocolResult<K> {
        let idx = get_bucket_index(key_bytes);
        let bkt = self.keys_bucket.get_mut(idx).unwrap();
        if bkt.contains(key) {
            let idx = idx + 1;
            for i in 1..=16 {
                if i >= idx {
                    let tmp = self.bucket_lens[i];
                    self.bucket_lens[i] = tmp - 1;
                }
            }
            self.keys_len -= 1;
            return bkt.remove_item(key);
        } else {
            Err(StoreError::GetNone.into())
        }
    }

    pub fn get_bucket(&self, index: usize) -> &Bucket<K> {
        self.keys_bucket
            .get(index)
            .expect("index must less than 16")
    }

    /// The function will panic when index is greater than or equal 16.
    pub fn get_abs_index_interval(&self, index: usize) -> (u32, u32) {
        (self.bucket_lens[index], self.bucket_lens[index + 1])
    }
}

pub struct Bucket<K: FixedCodec + PartialEq>(pub Vec<K>);

impl<K: FixedCodec + PartialEq> Bucket<K> {
    pub fn new() -> Self {
        Bucket(Vec::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn contains(&self, x: &K) -> bool {
        self.0.contains(x)
    }

    pub fn push(&mut self, value: K) {
        self.0.push(value);
    }

    pub fn remove_item(&mut self, key: &K) -> ProtocolResult<K> {
        let mut idx = self.len();
        for (i, item) in self.0.iter().enumerate() {
            if item == key {
                idx = i;
                break;
            }
        }

        if idx < self.len() {
            Ok(self.0.remove(idx))
        } else {
            Err(StoreError::GetNone.into())
        }
    }
}

impl<K: FixedCodec + PartialEq + PartialEq> rlp::Encodable for Bucket<K> {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        let inner: Vec<Vec<u8>> = self
            .0
            .iter()
            .map(|k| k.encode_fixed().expect("encode should not fail").to_vec())
            .collect();

        s.begin_list(1).append_list::<Vec<u8>, _>(&inner);
    }
}

impl<K: FixedCodec + PartialEq> rlp::Decodable for Bucket<K> {
    fn decode(r: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let inner_u8: Vec<Vec<u8>> = rlp::decode_list(r.at(0)?.as_raw());

        let inner_k: Result<Vec<K>, _> = inner_u8
            .into_iter()
            .map(|v| <_>::decode_fixed(Bytes::from(v)))
            .collect();

        let inner = inner_k.map_err(|_| rlp::DecoderError::Custom("decode K from bytes fail"))?;

        Ok(Bucket(inner))
    }
}

impl<K: FixedCodec + PartialEq> FixedCodec for Bucket<K> {
    fn encode_fixed(&self) -> ProtocolResult<Bytes> {
        Ok(Bytes::from(rlp::encode(self)))
    }

    fn decode_fixed(bytes: Bytes) -> ProtocolResult<Self> {
        Ok(rlp::decode(bytes.as_ref()).map_err(FixedCodecError::from)?)
    }
}

fn get_bucket_index(bytes: &Bytes) -> usize {
    (bytes[0] >> 4) as usize
}

#[derive(Debug, Display, From)]
pub enum StoreError {
    #[display(fmt = "the key not existed")]
    GetNone,

    #[display(fmt = "access array out of range")]
    OutRange,

    #[display(fmt = "decode error")]
    DecodeError,

    #[display(fmt = "overflow when calculating")]
    Overflow,
}

impl std::error::Error for StoreError {}

impl From<StoreError> for ProtocolError {
    fn from(err: StoreError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Binding, Box::new(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_empty_buckets() -> FixedBuckets<Bytes> {
        let bkts = (0..16).map(|_| Bucket::<Bytes>::new()).collect::<Vec<_>>();
        FixedBuckets::new(bkts)
    }

    #[test]
    fn test_insert() {
        let mut buckets = init_empty_buckets();
        assert!(buckets.is_empty());

        for i in 0..=255u8 {
            let key = Bytes::from(vec![i]);
            buckets.insert(get_bucket_index(&key), key);
        }

        let intervals = (0u32..=16).map(|i| i * 16).collect::<Vec<_>>();
        assert!(intervals == buckets.bucket_lens);
        assert!(buckets.keys_len == 256);

        for i in 0..16 {
            assert!(buckets.get_bucket(i).len() == 16);
        }

        let mut buckets = init_empty_buckets();
        for i in 0..8 {
            let key = Bytes::from(vec![i]);
            buckets.insert(get_bucket_index(&key), key);
        }

        assert!(buckets.get_bucket(0).len() == 8);
        assert!(buckets.keys_len == 8);
        for i in 1..16 {
            assert!(buckets.get_bucket(i).len() == 0);
        }
    }

    #[test]
    fn test_remove() {
        let mut buckets = init_empty_buckets();

        for i in 0..=255u8 {
            let key = Bytes::from(vec![i]);
            buckets.insert(get_bucket_index(&key), key);
        }

        let key = Bytes::from(vec![0]);
        let _ = buckets
            .remove_item(&key, &key.encode_fixed().unwrap())
            .unwrap();
        let intervals = (0u32..=16)
            .map(|i| if i == 0 { 0 } else { i * 16 - 1 })
            .collect::<Vec<_>>();
        assert!(buckets.keys_len == 255);
        assert!(intervals == buckets.bucket_lens);
    }

    #[test]
    fn test_contains() {
        let mut buckets = init_empty_buckets();

        for i in 0..3u8 {
            let key = Bytes::from(vec![i]);
            buckets.insert(get_bucket_index(&key), key);
        }

        let key = Bytes::from(vec![0]);
        assert!(buckets.contains(&key, &key.encode_fixed().unwrap()));

        let key = Bytes::from(vec![5]);
        assert!(!buckets.contains(&key, &key.encode_fixed().unwrap()));

        let key = Bytes::from(vec![20]);
        assert!(!buckets.contains(&key, &key.encode_fixed().unwrap()));
    }
}
