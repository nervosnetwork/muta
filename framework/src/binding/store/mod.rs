mod array;
mod map;
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
    pub keys_bucket:  Vec<Bucket<K>>,
    pub bucket_lens:  Vec<u64>,
    pub is_recovered: Vec<bool>,
}

impl<K: FixedCodec + PartialEq> FixedBuckets<K> {
    fn new() -> Self {
        let mut keys_bucket = Vec::new();
        let mut bucket_lens = vec![0];
        let mut is_recovered = Vec::new();

        for _i in 0..16 {
            keys_bucket.push(Bucket::new());
            bucket_lens.push(0u64);
            is_recovered.push(false);
        }

        FixedBuckets {
            keys_bucket,
            bucket_lens,
            is_recovered,
        }
    }

    fn recover_bucket(&mut self, index: usize, bucket: Bucket<K>) {
        self.keys_bucket[index] = bucket;
        self.is_recovered[index] = true;
        self.update_index_interval(index);
    }

    fn insert(&mut self, index: usize, key: K) {
        let bkt = self.keys_bucket.get_mut(index).unwrap();
        bkt.push(key);
        self.update_index_interval(index);
    }

    fn contains(&self, index: usize, key: &K) -> bool {
        self.keys_bucket[index].contains(key)
    }

    fn remove_item(&mut self, index: usize, key: &K) -> ProtocolResult<K> {
        let bkt = self.keys_bucket.get_mut(index).unwrap();
        if bkt.contains(key) {
            let val = bkt.remove_item(key)?;
            self.update_index_interval(index);
            Ok(val)
        } else {
            Err(StoreError::GetNone.into())
        }
    }

    fn get_bucket(&self, index: usize) -> &Bucket<K> {
        self.keys_bucket
            .get(index)
            .expect("index must less than 16")
    }

    /// The function will panic when index is greater than or equal 16.
    fn get_abs_index_interval(&self, index: usize) -> (u64, u64) {
        (self.bucket_lens[index], self.bucket_lens[index + 1])
    }

    fn is_bucket_recovered(&self, index: usize) -> bool {
        self.is_recovered[index]
    }

    fn update_index_interval(&mut self, index: usize) {
        let start = index + 1;
        let mut acc = self.bucket_lens[index];

        for i in start..17 {
            acc += self.keys_bucket[i - 1].len() as u64;
            self.bucket_lens[i] = acc;
        }
    }

    #[cfg(test)]
    fn len(&self) -> u64 {
        self.bucket_lens[16]
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct Bucket<K: FixedCodec + PartialEq>(Vec<K>);

impl<K: FixedCodec + PartialEq> Bucket<K> {
    fn new() -> Self {
        Bucket(Vec::new())
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn contains(&self, x: &K) -> bool {
        self.0.contains(x)
    }

    fn push(&mut self, value: K) {
        self.0.push(value);
    }

    fn remove_item(&mut self, key: &K) -> ProtocolResult<K> {
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

#[inline(always)]
fn get_bucket_index(bytes: &Bytes) -> usize {
    let len = bytes.len() - 1;
    (bytes[len] >> 4) as usize
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

    #[test]
    fn test_insert() {
        let mut buckets = FixedBuckets::new();
        assert!(buckets.is_empty());

        for i in 0..=255u8 {
            let key = Bytes::from(vec![i]);
            buckets.insert(get_bucket_index(&key), key);
        }

        println!("{:?}", buckets.bucket_lens);

        let intervals = (0u64..=16).map(|i| i * 16).collect::<Vec<_>>();
        assert!(intervals == buckets.bucket_lens);
        assert!(buckets.len() == 256);

        for i in 0..16 {
            assert!(buckets.get_bucket(i).len() == 16);
        }

        let mut buckets = FixedBuckets::new();
        for i in 0..8 {
            let key = Bytes::from(vec![i]);
            buckets.insert(get_bucket_index(&key), key);
        }

        assert!(buckets.get_bucket(0).len() == 8);
        assert!(buckets.len() == 8);
        for i in 1..16 {
            assert!(buckets.get_bucket(i).len() == 0);
        }
    }

    #[test]
    fn test_remove() {
        let mut buckets = FixedBuckets::new();

        for i in 0..=255u8 {
            let key = Bytes::from(vec![i]);
            buckets.insert(get_bucket_index(&key), key);
        }

        let key = Bytes::from(vec![0]);
        let _ = buckets
            .remove_item(get_bucket_index(&key.encode_fixed().unwrap()), &key)
            .unwrap();
        let intervals = (0u64..=16)
            .map(|i| if i == 0 { 0 } else { i * 16 - 1 })
            .collect::<Vec<_>>();
        assert!(buckets.len() == 255);
        assert!(intervals == buckets.bucket_lens);
    }

    #[test]
    fn test_contains() {
        let mut buckets = FixedBuckets::new();

        for i in 0..3u8 {
            let key = Bytes::from(vec![i]);
            buckets.insert(get_bucket_index(&key), key);
        }

        let key = Bytes::from(vec![0]);
        assert!(buckets.contains(get_bucket_index(&key.encode_fixed().unwrap()), &key));

        let key = Bytes::from(vec![5]);
        assert!(!buckets.contains(get_bucket_index(&key.encode_fixed().unwrap()), &key));

        let key = Bytes::from(vec![20]);
        assert!(!buckets.contains(get_bucket_index(&key.encode_fixed().unwrap()), &key));
    }
}
