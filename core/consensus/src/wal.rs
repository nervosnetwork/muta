use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use protocol::codec::ProtocolCodecSync;
use protocol::types::{Bytes, Hash, SignedTransaction};
use protocol::ProtocolResult;

use crate::fixed_types::FixedSignedTxs;
use crate::ConsensusError;

#[derive(Debug)]
pub struct SignedTxsWAL {
    path: PathBuf,
}

impl SignedTxsWAL {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        if !path.as_ref().exists() {
            fs::create_dir_all(&path).expect("Failed to create wal directory");
        }

        SignedTxsWAL {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn save(
        &self,
        height: u64,
        block_hash: Hash,
        txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let mut dir = self.path.clone();
        dir.push(height.to_string());
        if !Path::new(&dir).exists() {
            fs::create_dir(&dir).map_err(ConsensusError::WALErr)?;
        }

        dir.push(block_hash.as_hex());
        dir.set_extension("txt");

        let mut wal_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(dir)
            .map_err(ConsensusError::WALErr)?;

        let data = FixedSignedTxs::new(txs).encode_sync()?;
        wal_file
            .write_all(data.as_ref())
            .map_err(ConsensusError::WALErr)?;
        Ok(())
    }

    pub fn load(&self, height: u64, block_hash: Hash) -> ProtocolResult<Vec<SignedTransaction>> {
        let mut file_path = self.path.clone();
        file_path.push(height.to_string());
        file_path.push(block_hash.as_hex());
        file_path.set_extension("txt");

        let mut read_buf = Vec::new();
        let mut file = fs::File::open(&file_path).map_err(ConsensusError::WALErr)?;
        let _ = file
            .read_to_end(&mut read_buf)
            .map_err(ConsensusError::WALErr)?;
        let txs = FixedSignedTxs::decode_sync(Bytes::from(read_buf))?;
        Ok(txs.inner)
    }

    pub fn remove(&self, committed_height: u64) -> ProtocolResult<()> {
        for entry in fs::read_dir(&self.path).map_err(ConsensusError::WALErr)? {
            let folder = entry.map_err(ConsensusError::WALErr)?.path();
            let folder_name = folder
                .file_stem()
                .ok_or_else(|| ConsensusError::Other("file stem error".to_string()))?
                .to_os_string()
                .clone();
            let folder_name = folder_name.into_string().map_err(|err| {
                ConsensusError::Other(format!("transfer os string to string error {:?}", err))
            })?;
            let height = folder_name.parse::<u64>().map_err(|err| {
                ConsensusError::Other(format!("parse folder name {:?} error {:?}", folder, err))
            })?;

            if height <= committed_height {
                fs::remove_dir_all(folder).map_err(ConsensusError::WALErr)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    extern crate test;

    use rand::random;
    use test::Bencher;

    use protocol::types::{Hash, RawTransaction, TransactionRequest};
    use protocol::Bytes;

    use super::*;

    static FULL_TXS_PATH: &str = "./devtools/chain/data";

    pub fn mock_hash() -> Hash {
        Hash::digest(get_random_bytes(10))
    }

    pub fn mock_raw_tx() -> RawTransaction {
        RawTransaction {
            chain_id:     mock_hash(),
            nonce:        mock_hash(),
            timeout:      100,
            cycles_price: 1,
            cycles_limit: 100,
            request:      mock_transaction_request(),
        }
    }

    pub fn mock_transaction_request() -> TransactionRequest {
        TransactionRequest {
            service_name: "mock-service".to_owned(),
            method:       "mock-method".to_owned(),
            payload:      "mock-payload".to_owned(),
        }
    }

    pub fn mock_sign_tx() -> SignedTransaction {
        SignedTransaction {
            raw:       mock_raw_tx(),
            tx_hash:   mock_hash(),
            pubkey:    Default::default(),
            signature: Default::default(),
        }
    }

    pub fn mock_wal_txs() -> Vec<SignedTransaction> {
        (0..20000).map(|_| mock_sign_tx()).collect::<Vec<_>>()
    }

    pub fn get_random_bytes(len: usize) -> Bytes {
        let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
        Bytes::from(vec)
    }

    #[test]
    fn test_txs_wal() {
        let wal = SignedTxsWAL::new(FULL_TXS_PATH.to_string());
        let txs_01 = mock_wal_txs();
        let hash_01 = Hash::digest(Bytes::from(rlp::encode_list(&txs_01)));
        wal.save(1u64, hash_01.clone(), txs_01.clone()).unwrap();
        let txs_02 = mock_wal_txs();
        let hash_02 = Hash::digest(Bytes::from(rlp::encode_list(&txs_02)));
        wal.save(3u64, hash_02.clone(), txs_02.clone()).unwrap();

        assert_eq!(wal.load(1u64, hash_01.clone()).unwrap(), txs_01);
        assert_eq!(wal.load(3u64, hash_02.clone()).unwrap(), txs_02);

        wal.remove(2u64).unwrap();
        assert!(wal.load(1u64, hash_01).is_err());
        assert!(wal.load(2u64, hash_02).is_err());
    }

    #[test]
    fn test_wal_txs_codec() {
        for _ in 0..10 {
            let txs = FixedSignedTxs::new(mock_wal_txs());
            assert_eq!(
                FixedSignedTxs::decode_sync(txs.encode_sync().unwrap()).unwrap(),
                txs
            );
        }
    }

    #[bench]
    fn bench_txs_rlp_encode(b: &mut Bencher) {
        let txs = mock_wal_txs();

        b.iter(move || {
            let _ = rlp::encode_list(&txs);
        });
    }

    #[bench]
    fn bench_txs_prost_encode(b: &mut Bencher) {
        let txs = FixedSignedTxs::new(mock_wal_txs());

        b.iter(move || {
            let _ = txs.encode_sync();
        });
    }
}
