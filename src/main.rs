#![feature(async_await)]

use std::sync::Arc;

use core_storage::{adapter::rocks::RocksAdapter, ImplStorage};
use protocol::traits::Storage;
use protocol::ProtocolError;

#[runtime::main]
async fn main() -> Result<(), ProtocolError> {
    let storage = ImplStorage::new(Arc::new(RocksAdapter::new(
        "./devtools/data/storage".to_owned(),
    )?));
    storage.insert_transactions(vec![]).await?;

    Ok(())
}
