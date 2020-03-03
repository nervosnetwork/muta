use std::fs;
use std::io::{Read, Write};

use protocol::fixed_codec::FixedCodec;
use protocol::types::{Hash, WalSaveTxs};
use protocol::{Bytes, ProtocolResult};

use crate::ConsensusError;

#[derive(Debug)]
pub struct FullTxsWal {
    path: String,
}

impl FullTxsWal {
    pub fn new(path: String) -> Self {
        if fs::read_dir(&path).is_err() {
            fs::create_dir(&path).expect("Failed to create wal directory");
        }

        FullTxsWal { path }
    }

    pub fn save_txs(&self, height: u64, block_hash: Hash, txs: WalSaveTxs) -> ProtocolResult<()> {
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
            .write_all(txs.encode_fixed()?.as_ref())
            .map_err(ConsensusError::WalErr)?;
        Ok(())
    }

    pub fn load_txs(&self, height: u64, block_hash: Hash) -> ProtocolResult<WalSaveTxs> {
        let file_path =
            self.path.clone() + "/" + &height.to_string() + "/" + &block_hash.as_hex() + ".txt";
        let mut read_buf = Vec::new();
        let mut file = fs::File::open(&file_path).map_err(ConsensusError::WalErr)?;
        let _ = file
            .read_to_end(&mut read_buf)
            .map_err(ConsensusError::WalErr)?;
        let txs: WalSaveTxs = FixedCodec::decode_fixed(Bytes::from(read_buf))?;
        Ok(txs)
    }

    pub fn remove(&self, exec_height: u64) -> ProtocolResult<()> {
        for entry in fs::read_dir(&self.path).map_err(ConsensusError::WalErr)? {
            let folder = entry.map_err(ConsensusError::WalErr)?.path();
            let folder_name = folder
                .file_stem()
                .ok_or_else(|| ConsensusError::Other("file stem".to_string()))?
                .to_os_string()
                .clone();
            let folder_name = folder_name
                .into_string()
                .map_err(|err| ConsensusError::Other(format!("{:?}", err)))?;
            let height = folder_name
                .parse::<u64>()
                .map_err(|err| ConsensusError::Other(format!("{:?}", err)))?;

            if height < exec_height {
                fs::remove_dir_all(folder).map_err(ConsensusError::WalErr)?;
            }
        }
        Ok(())
    }
}
