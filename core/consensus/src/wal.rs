use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use common_apm::muta_apm;
use protocol::codec::ProtocolCodecSync;
use protocol::types::{Bytes, Hash, SignedTransaction};
use protocol::ProtocolResult;

use crate::fixed_types::FixedSignedTxs;
use crate::ConsensusError;
use bytes::{BufMut, BytesMut};
use creep::Context;
use std::str::FromStr;
use std::time::SystemTime;

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
        ordered_signed_transactions_hash: Hash,
        txs: Vec<SignedTransaction>,
    ) -> ProtocolResult<()> {
        let mut wal_path = self.path.clone();
        wal_path.push(height.to_string());
        if !wal_path.exists() {
            fs::create_dir(&wal_path).map_err(ConsensusError::WALErr)?;
        }

        wal_path.push(ordered_signed_transactions_hash.as_hex());
        wal_path.set_extension("txt");

        let mut wal_file = match fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(wal_path)
        {
            Ok(file) => file,
            Err(err) => {
                if err.kind() == ErrorKind::AlreadyExists {
                    return Ok(());
                } else {
                    return Err(ConsensusError::WALErr(err).into());
                }
            }
        };

        let data = FixedSignedTxs::new(txs).encode_sync()?;
        wal_file
            .write_all(data.as_ref())
            .map_err(ConsensusError::WALErr)?;
        Ok(())
    }

    pub fn available_height(&self) -> ProtocolResult<Vec<u64>> {
        let dir_path = self.path.clone();
        let mut availables = vec![];
        for item in fs::read_dir(dir_path).map_err(ConsensusError::WALErr)? {
            let item = item.map_err(ConsensusError::WALErr)?;

            if item.path().is_dir() {
                availables.push(item.file_name().to_str().unwrap().parse().unwrap())
            }
        }
        Ok(availables)
    }

    pub fn remove_all(&self) -> ProtocolResult<()> {
        for height in self.available_height()? {
            self.remove(height)?
        }
        Ok(())
    }

    pub fn load(
        &self,
        height: u64,
        ordered_signed_transactions_hash: Hash,
    ) -> ProtocolResult<Vec<SignedTransaction>> {
        let mut file_path = self.path.clone();
        file_path.push(height.to_string());
        file_path.push(ordered_signed_transactions_hash.as_hex());
        file_path.set_extension("txt");

        self.recover_stxs(file_path)
    }

    pub fn load_by_height(&self, height: u64) -> Vec<SignedTransaction> {
        let mut dir = self.path.clone();
        dir.push(height.to_string());
        let dir = if let Ok(res) = fs::read_dir(dir) {
            res
        } else {
            return Vec::new();
        };

        let mut ret = Vec::new();
        for entry in dir {
            if let Ok(file_dir) = entry {
                if let Ok(mut stxs) = self.recover_stxs(file_dir.path()) {
                    ret.append(&mut stxs);
                }
            }
        }
        ret
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

    fn recover_stxs(&self, file_path: PathBuf) -> ProtocolResult<Vec<SignedTransaction>> {
        let mut read_buf = Vec::new();
        let mut file = fs::File::open(&file_path).map_err(ConsensusError::WALErr)?;
        let _ = file
            .read_to_end(&mut read_buf)
            .map_err(ConsensusError::WALErr)?;
        let txs = FixedSignedTxs::decode_sync(Bytes::from(read_buf))?;
        Ok(txs.inner)
    }
}

#[derive(Debug)]
pub struct ConsensusWal {
    path: PathBuf,
}

impl ConsensusWal {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        if !path.as_ref().exists() {
            fs::create_dir_all(&path).expect("Failed to create wal directory");
        }

        ConsensusWal {
            path: path.as_ref().to_path_buf(),
        }
    }

    #[muta_apm::derive::tracing_span(kind = "consensus_wal")]
    pub fn update_overlord_wal(&self, ctx: Context, info: Bytes) -> ProtocolResult<()> {
        // 1st, make sure the dir exists
        let dir_path = self.path.clone();
        if !dir_path.exists() {
            fs::create_dir(&dir_path).map_err(ConsensusError::WALErr)?;
        }

        // 2nd, write info into file
        let check_sum = Hash::digest(info.clone());

        let mut content = BytesMut::new();
        content.put(check_sum.as_bytes());
        content.put(info);

        let (data_path, timestamp) = {
            loop {
                let timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(ConsensusError::SystemTime)?;

                let timestamp = timestamp.as_millis();

                let mut data_path = dir_path.clone();

                data_path.push(timestamp.to_string());

                if !data_path.exists() {
                    break (data_path, timestamp);
                }
            }
        };

        let mut data_file = match fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(data_path)
        {
            Ok(file) => file,
            Err(err) => {
                if err.kind() == ErrorKind::AlreadyExists {
                    return Ok(());
                } else {
                    return Err(ConsensusError::WALErr(err).into());
                }
            }
        };

        data_file
            .write_all(content.as_ref())
            .map_err(ConsensusError::WALErr)?;

        // 3rd, we can safely clean other old wal files
        for item in fs::read_dir(dir_path).map_err(ConsensusError::WALErr)? {
            let item = item.map_err(ConsensusError::WALErr)?;

            let file_name = item
                .file_name()
                .to_str()
                .ok_or(ConsensusError::FileNameTimestamp)?
                .to_owned();

            let file_name_timestamp = u128::from_str(file_name.as_str())
                .map_err(|e| ConsensusError::FileNameTimestamp)?;

            if file_name_timestamp < timestamp {
                fs::remove_file(item.path()).map_err(ConsensusError::WALErr)?;
            }
        }

        Ok(())
    }

    #[muta_apm::derive::tracing_span(kind = "consensus_wal")]
    pub fn load_overlord_wal(&self, ctx: Context) -> ProtocolResult<Bytes> {
        // 1st,
        let dir_path = self.path.clone();
        if !dir_path.exists() {
            return Err(ConsensusError::ConsensusWalDirNotExist.into());
        }

        // 2 read all log files and sort by timestamp in their names
        let files = fs::read_dir(dir_path.clone()).map_err(ConsensusError::WALErr)?;

        let mut file_names_timestamps = files
            .filter_map(|item| {
                let item = item.ok()?;
                let file_name = item.file_name();
                let file_name = file_name.to_str()?;

                let file_name_timestamp = u128::from_str(file_name).ok()?;

                Some(file_name_timestamp)
            })
            .collect::<Vec<_>>();

        file_names_timestamps.sort_by_key(|&b| std::cmp::Reverse(b));

        // 3rd, get a latest and valid wal if possible
        let mut index = 0;
        let content = loop {
            if index >= file_names_timestamps.len() {
                break None;
            }

            let file_name_timestamp = file_names_timestamps[index];

            let mut log_path = dir_path.clone();
            log_path.push(file_name_timestamp.to_string());

            let mut read_buf = Vec::new();
            let mut file = fs::File::open(&log_path).map_err(ConsensusError::WALErr)?;
            let res = file.read_to_end(&mut read_buf);
            if res.is_err() {
                continue;
            }

            let mut info = Bytes::from(read_buf);

            if info.len() < Hash::default().as_bytes().len() {
                continue;
            }

            let content = info.split_off(Hash::default().as_bytes().len());

            if info == Hash::digest(content.clone()).as_bytes() {
                break Some(content);
            } else {
                index += 1;
            }
        };

        content.ok_or_else(|| ConsensusError::ConsensusWalNoWalFile.into())
    }

    pub fn clear(&self) -> ProtocolResult<()> {
        let dir_path = self.path.clone();
        if !dir_path.exists() {
            return Ok(());
        }

        for item in fs::read_dir(dir_path).map_err(ConsensusError::WALErr)? {
            let item = item.map_err(ConsensusError::WALErr)?;

            fs::remove_file(item.path()).map_err(ConsensusError::WALErr)?;
        }
        Ok(())
    }
}

#[rustfmt::skip]
/// Bench in Intel(R) Core(TM) i7-4770HQ CPU @ 2.20GHz (8 x 2200):
/// test wal::test::bench_save_wal_1000_txs  ... bench:   2,346,611 ns/iter (+/- 754,074)
/// test wal::test::bench_save_wal_16000_txs ... bench:  41,576,328 ns/iter (+/- 2,547,323)
/// test wal::test::bench_save_wal_2000_txs  ... bench:   4,759,015 ns/iter (+/- 460,748)
/// test wal::test::bench_save_wal_4000_txs  ... bench:   9,725,284 ns/iter (+/- 452,143)
/// test wal::test::bench_save_wal_8000_txs  ... bench:  19,971,012 ns/iter (+/- 1,620,755)
/// test wal::test::bench_save_wal_16000_txs ... bench:  41,576,328 ns/iter (+/- 2,547,323)
/// test wal::test::bench_txs_prost_encode   ... bench:  40,020,365 ns/iter (+/- 2,800,361)
/// test wal::test::bench_txs_rlp_encode     ... bench:  40,792,370 ns/iter (+/- 1,908,695)

#[cfg(test)]
mod tests {
    extern crate test;

    use rand::random;
    use test::Bencher;

    use protocol::types::{Address, Hash, RawTransaction, TransactionRequest};
    use protocol::Bytes;

    use super::*;

    static FULL_TXS_PATH: &str = "./free-space/wal/txs";

    static FULL_CONSENSUS_PATH: &str = "./free-space/wal/consensus";
    
    fn mock_hash() -> Hash {
        Hash::digest(get_random_bytes(10))
    }

    fn mock_address() -> Address {
        let hash = mock_hash();
        Address::from_hash(hash).unwrap()
    }

    fn mock_raw_tx() -> RawTransaction {
        RawTransaction {
            chain_id:     mock_hash(),
            nonce:        mock_hash(),
            timeout:      100,
            cycles_price: 1,
            cycles_limit: 100,
            request:      mock_transaction_request(),
            sender: mock_address(),
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

    pub fn mock_wal_txs(size: usize) -> Vec<SignedTransaction> {
        (0..size).map(|_| mock_sign_tx()).collect::<Vec<_>>()
    }

    pub fn get_random_bytes(len: usize) -> Bytes {
        let vec: Vec<u8> = (0..len).map(|_| random::<u8>()).collect();
        Bytes::from(vec)
    }

    #[test]
    fn test_txs_wal() {
        fs::remove_dir_all(PathBuf::from_str(FULL_TXS_PATH).unwrap()).unwrap();
        
        let wal = SignedTxsWAL::new(FULL_TXS_PATH.to_string());
        let txs_01 = mock_wal_txs(100);
        let hash_01 = Hash::digest(Bytes::from(rlp::encode_list(&txs_01)));
        wal.save(1u64, hash_01.clone(), txs_01.clone()).unwrap();
        let txs_02 = mock_wal_txs(100);
        let hash_02 = Hash::digest(Bytes::from(rlp::encode_list(&txs_02)));
        wal.save(3u64, hash_02.clone(), txs_02.clone()).unwrap();

        let txs_03 = mock_wal_txs(100);
        let hash_03 = Hash::digest(Bytes::from(rlp::encode_list(&txs_03)));
        wal.save(3u64, hash_03, txs_03.clone()).unwrap();

        let res = wal.load_by_height(3);
        assert_eq!(res.len(), 200);

        for tx in res.iter() {
            assert!(txs_02.contains(tx) || txs_03.contains(tx));
        }

        assert_eq!(wal.load(1u64, hash_01.clone()).unwrap(), txs_01);
        assert_eq!(wal.load(3u64, hash_02.clone()).unwrap(), txs_02);

        wal.remove(2u64).unwrap();
        assert!(wal.load(1u64, hash_01).is_err());
        assert!(wal.load(2u64, hash_02).is_err());

        wal.remove(1u64).unwrap();
        wal.remove(3u64).unwrap();
    }

    #[test]
    fn test_consensus_wal() {
        // write one, read one
        let wal = ConsensusWal::new(FULL_CONSENSUS_PATH.to_string());
        let info = get_random_bytes(1000);
        wal.update_overlord_wal(Context::new(),info.clone()).unwrap();
        
        let load = wal.load_overlord_wal(Context::new()).unwrap();
        assert_eq!(load,info);
        
        // write three, read latest
        fs::remove_dir_all(PathBuf::from_str(FULL_CONSENSUS_PATH).unwrap()).unwrap();

        let info = get_random_bytes(1000);
        wal.update_overlord_wal(Context::new(),get_random_bytes(1000)).unwrap();
        wal.update_overlord_wal(Context::new(),get_random_bytes(1000)).unwrap();
        wal.update_overlord_wal(Context::new(),info.clone()).unwrap();

        let load = wal.load_overlord_wal(Context::new()).unwrap();
        assert_eq!(load,info);

        // remove all, read nothing
        fs::remove_dir_all(PathBuf::from_str(FULL_CONSENSUS_PATH).unwrap()).unwrap();

        let load = wal.load_overlord_wal(Context::new());
        assert!(load.is_err());
        
        // write a old correct one and a new wrong one, read old
        
        // old one
        //fs::remove_dir_all(PathBuf::from_str(FULL_CONSENSUS_PATH).unwrap()).unwrap();

        let info = get_random_bytes(1000);
        wal.update_overlord_wal(Context::new(),info.clone()).unwrap();
        
        // -> copy and modify to a new fake one

        let mut files = fs::read_dir(FULL_CONSENSUS_PATH).unwrap();
        
        let file = files.next().unwrap().unwrap();
        
        let from = u128::from_str( file.file_name().to_str().unwrap()).unwrap();

        let to = file.path().parent().unwrap().join((from+1).to_string());
        
        let mut new_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(to).unwrap();

        new_file
            .write_all(get_random_bytes(1000).as_ref()).unwrap();

        let load = wal.load_overlord_wal(Context::new()).unwrap();
        assert_eq!(load,info);

        fs::remove_dir_all(PathBuf::from_str(FULL_CONSENSUS_PATH).unwrap()).unwrap();
    }
    
    #[test]
    fn test_wal_txs_codec() {
        for _ in 0..10 {
            let txs = FixedSignedTxs::new(mock_wal_txs(100));
            assert_eq!(
                FixedSignedTxs::decode_sync(txs.encode_sync().unwrap()).unwrap(),
                txs
            );
        }
    }

    #[bench]
    fn bench_txs_rlp_encode(b: &mut Bencher) {
        let txs = mock_wal_txs(20000);

        b.iter(move || {
            let _ = rlp::encode_list(&txs);
        });
    }

    #[bench]
    fn bench_txs_prost_encode(b: &mut Bencher) {
        let txs = FixedSignedTxs::new(mock_wal_txs(20000));

        b.iter(move || {
            let _ = txs.encode_sync();
        });
    }

    #[bench]
    fn bench_save_wal_1000_txs(b: &mut Bencher) {
        let wal = SignedTxsWAL::new(FULL_TXS_PATH.to_string());
        let txs = mock_wal_txs(1000);
        let txs_hash = Hash::digest(Bytes::from(rlp::encode_list(&txs)));

        b.iter(move || {
            wal.save(1u64, txs_hash.clone(), txs.clone()).unwrap();
        })
    }

    #[bench]
    fn bench_save_wal_2000_txs(b: &mut Bencher) {
        let wal = SignedTxsWAL::new(FULL_TXS_PATH.to_string());
        let txs = mock_wal_txs(2000);
        let txs_hash = Hash::digest(Bytes::from(rlp::encode_list(&txs)));

        b.iter(move || {
            wal.save(1u64, txs_hash.clone(), txs.clone()).unwrap();
        })
    }

    #[bench]
    fn bench_save_wal_4000_txs(b: &mut Bencher) {
        let wal = SignedTxsWAL::new(FULL_TXS_PATH.to_string());
        let txs = mock_wal_txs(4000);
        let txs_hash = Hash::digest(Bytes::from(rlp::encode_list(&txs)));

        b.iter(move || {
            wal.save(1u64, txs_hash.clone(), txs.clone()).unwrap();
        })
    }

    #[bench]
    fn bench_save_wal_8000_txs(b: &mut Bencher) {
        let wal = SignedTxsWAL::new(FULL_TXS_PATH.to_string());
        let txs = mock_wal_txs(8000);
        let txs_hash = Hash::digest(Bytes::from(rlp::encode_list(&txs)));

        b.iter(move || {
            wal.save(1u64, txs_hash.clone(), txs.clone()).unwrap();
        })
    }

    #[bench]
    fn bench_save_wal_16000_txs(b: &mut Bencher) {
        let wal = SignedTxsWAL::new(FULL_TXS_PATH.to_string());
        let txs = mock_wal_txs(16000);
        let txs_hash = Hash::digest(Bytes::from(rlp::encode_list(&txs)));

        b.iter(move || {
            wal.save(1u64, txs_hash.clone(), txs.clone()).unwrap();
        })
    }
}
