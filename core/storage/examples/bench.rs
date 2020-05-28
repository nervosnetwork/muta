use core_storage::{adapter::rocks::RocksAdapter, CommonHashKey, ImplStorage};
use protocol::{
    traits::{Context, Storage},
    types::{Hash, RawTransaction, SignedTransaction, TransactionRequest},
    Bytes,
};

use std::{
    fs::OpenOptions,
    io::prelude::*,
    io::{BufReader, LineWriter},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::Instant,
};

const NUMBER_OF_TXS_PER_ROUND: usize = 15_000; // 1.5W, 2.5M

#[tokio::main]
pub async fn main() {
    if std::env::args().nth(1) == Some("generate".to_string()) {
        println!("generate 1.5W txs");

        let mut height = 1u64;
        let mut count = std::env::args()
            .nth(2)
            .expect("number of round(1.5W txs per round, 2.5M)")
            .parse::<u64>()
            .expect("number of round(1.5W txs per round, 2.5M)");

        let db_path = std::env::args().nth(3).expect("db patch");
        let max_fd = std::env::args()
            .nth(4)
            .expect("max open files for rocksdb")
            .parse::<i32>()
            .expect("max open files for rocksdb");

        let mut hash_keys_file = {
            let mut file_path = PathBuf::from(db_path.clone());
            file_path.push("hash_keys");

            let file = OpenOptions::new()
                .write(true)
                .append(true)
                .create_new(true)
                .open(file_path)
                .expect("tx hashes file");

            LineWriter::new(file)
        };

        let adapter = RocksAdapter::new(db_path, max_fd).expect("create adapter");
        let storage = ImplStorage::new(Arc::new(adapter));

        let mut hash_keys = Vec::with_capacity(NUMBER_OF_TXS_PER_ROUND);

        while count > 0 {
            let stxs = (0..NUMBER_OF_TXS_PER_ROUND)
                .map(|_| {
                    let bytes = get_random_bytes();
                    let hash = Hash::digest(bytes);

                    hash_keys.push(CommonHashKey::new(height, hash.clone()));
                    mock_signed_tx(hash)
                })
                .collect::<Vec<_>>();

            for key in hash_keys.drain(..) {
                let encoded_key = key.to_string();
                hash_keys_file
                    .write_all(encoded_key.as_bytes())
                    .expect("write tx hash");
                hash_keys_file.write_all(b"\n").expect("write line");
            }

            storage
                .insert_transactions(Context::new(), height, stxs)
                .await
                .expect("insert transaction");

            count -= 1;
            height += 1;
        }

        println!("insert complete, height {}", height - 1);
    } else if std::env::args().nth(1) == Some("fetch".to_string()) {
        let db_path = std::env::args().nth(2).expect("db patch");
        let max_fd = std::env::args()
            .nth(3)
            .expect("max open files for rocksdb")
            .parse::<i32>()
            .expect("max open files for rocksdb");
        let height = std::env::args()
            .nth(4)
            .expect("height")
            .parse::<u64>()
            .expect("height");

        let hash_keys_file = {
            let mut file_path = PathBuf::from(db_path.clone());
            file_path.push("hash_keys");

            let file = OpenOptions::new()
                .read(true)
                .open(file_path)
                .expect("tx hashes file");

            BufReader::new(file).lines()
        };

        let hashes = hash_keys_file
            .skip((height - 1) as usize * NUMBER_OF_TXS_PER_ROUND)
            .take(NUMBER_OF_TXS_PER_ROUND)
            .map(|l| {
                let key = CommonHashKey::from_str(&l.expect("read line")).expect("key");
                key.hash().to_owned()
            })
            .collect::<Vec<_>>();

        let adapter = RocksAdapter::new(db_path, max_fd).expect("create adapter");
        let storage = ImplStorage::new(Arc::new(adapter));

        let now = Instant::now();
        let stxs = storage
            .get_transactions(Context::new(), height, hashes)
            .await
            .expect("fetch");

        println!("total {}, fetch {}", NUMBER_OF_TXS_PER_ROUND, stxs.len());
        println!("fetch cost {} ms", now.elapsed().as_millis());
    } else {
        println!(
            r#"
        Usage:
            generate [round] [db path] [fd]

            fetch [db path] [fd] [height]
        "#
        );
    }
}

fn get_random_bytes() -> Bytes {
    let mut buf = [0u8; 32];
    for u in &mut buf {
        *u = rand::random::<u8>();
    }

    Bytes::copy_from_slice(&buf)
}

fn mock_signed_tx(tx_hash: Hash) -> SignedTransaction {
    let nonce = Hash::digest(Bytes::from("XXXX"));

    let request = TransactionRequest {
        service_name: "test".to_owned(),
        method:       "test".to_owned(),
        payload:      "test".to_owned(),
    };

    let raw = RawTransaction {
        chain_id: nonce.clone(),
        nonce,
        timeout: 10,
        cycles_limit: 10,
        cycles_price: 1,
        request,
    };

    SignedTransaction {
        raw,
        tx_hash,
        pubkey: Default::default(),
        signature: Default::default(),
    }
}
