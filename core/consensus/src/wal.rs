use std::fs;
use std::io::{Read, Write};

use protocol::types::{Hash, SignedTransaction};
use protocol::ProtocolResult;

use crate::ConsensusError;

#[derive(Debug)]
pub struct FullTxsWal {
    path: String,
}

impl FullTxsWal {
    pub fn new(path: String) -> Self {
        if fs::read_dir(&path).is_err() {
            fs::create_dir_all(&path).expect("Failed to create wal directory");
        }

        FullTxsWal { path }
    }

    pub fn save_txs(
        &self,
        height: u64,
        block_hash: Hash,
        txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let dir = self.path.clone() + "/" + &height.to_string();
        if fs::read_dir(&dir).is_err() {
            fs::create_dir(&dir).map_err(ConsensusError::WalErr)?;
        }

        let file_path = dir + "/" + &block_hash.as_hex() + ".txt";
        let mut wal_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)
            .map_err(ConsensusError::WalErr)?;

        wal_file
            .write_all(&rlp::encode_list(&txs))
            .map_err(ConsensusError::WalErr)?;
        Ok(())
    }

    pub fn load_txs(
        &self,
        height: u64,
        block_hash: Hash,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let file_path =
            self.path.clone() + "/" + &height.to_string() + "/" + &block_hash.as_hex() + ".txt";
        let mut read_buf = Vec::new();
        let mut file = fs::File::open(&file_path).map_err(ConsensusError::WalErr)?;
        let _ = file
            .read_to_end(&mut read_buf)
            .map_err(ConsensusError::WalErr)?;
        let txs: Vec<SignedTransaction> = rlp::decode_list(&read_buf);
        Ok(txs)
    }

    pub fn remove(&self, exec_height: u64) -> ProtocolResult<()> {
        for entry in fs::read_dir(&self.path).map_err(ConsensusError::WalErr)? {
            let folder = entry.map_err(ConsensusError::WalErr)?.path();
            let folder_name = folder
                .file_stem()
                .ok_or_else(|| ConsensusError::Other("file stem error".to_string()))?
                .to_os_string()
                .clone();
            let folder_name = folder_name.into_string().map_err(|err| {
                ConsensusError::Other(format!("transfer os string to string error {:?}", err))
            })?;
            let height = folder_name.parse::<u64>().map_err(|err| {
                ConsensusError::Other(format!("parse folder name string error {:?}", err))
            })?;

            if height < exec_height {
                fs::remove_dir_all(folder).map_err(ConsensusError::WalErr)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use rand::random;

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
        (0..5000).map(|_| mock_sign_tx()).collect::<Vec<_>>()
    }

    pub fn get_random_bytes(len: usize) -> Bytes {
        let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
        Bytes::from(vec)
    }

    #[test]
    fn test_txs_wal() {
        let wal = FullTxsWal::new(FULL_TXS_PATH.to_string());
        let txs_01 = mock_wal_txs();
        let hash_01 = Hash::digest(Bytes::from(rlp::encode_list(&txs_01)));
        wal.save_txs(1u64, hash_01.clone(), txs_01.clone()).unwrap();
        let txs_02 = mock_wal_txs();
        let hash_02 = Hash::digest(Bytes::from(rlp::encode_list(&txs_02)));
        wal.save_txs(3u64, hash_02.clone(), txs_02.clone()).unwrap();

        assert_eq!(wal.load_txs(1u64, hash_01.clone()).unwrap(), txs_01);
        assert_eq!(wal.load_txs(3u64, hash_02.clone()).unwrap(), txs_02);

        wal.remove(2u64).unwrap();
        assert!(wal.load_txs(1u64, hash_01).is_err());
        assert!(wal.load_txs(2u64, hash_02).is_err());
    }
}
