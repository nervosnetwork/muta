use futures::executor::block_on;

use core_context::Context;
use core_runtime::{DataCategory, Database, DatabaseError};

fn get_block<K: AsRef<[u8]>, D: Database>(
    db: &D,
    ctx: Context,
    key: K,
) -> Result<Option<Vec<u8>>, DatabaseError> {
    let block = block_on(db.get(ctx, DataCategory::Block, key.as_ref()))?;

    Ok(block)
}

pub fn test_get<D: Database>(db: &D) {
    let ctx = Context::new();
    let data = b"test".to_vec();

    assert_eq!(get_block(db, ctx.clone(), "test"), Ok(None));

    block_on(db.insert(ctx.clone(), DataCategory::Block, data.clone(), data.clone())).unwrap();
    assert_eq!(get_block(db, ctx, "test"), Ok(Some(data)));
}

pub fn test_insert<D: Database>(db: &D) {
    let ctx = Context::new();
    let data = b"test".to_vec();

    block_on(db.insert(ctx.clone(), DataCategory::Block, data.clone(), data.clone())).unwrap();
    assert_eq!(get_block(db, ctx, "test"), Ok(Some(data)));
}

pub fn test_insert_batch<D: Database>(db: &D) {
    let ctx = Context::new();
    let data1 = b"test1".to_vec();
    let data2 = b"test2".to_vec();

    block_on(db.insert_batch(
        ctx.clone(),
        DataCategory::Block,
        vec![data1.clone(), data2.clone()],
        vec![data1.clone(), data2.clone()],
    ))
    .unwrap();

    assert_eq!(get_block(db, ctx.clone(), data1.clone()), Ok(Some(data1)));
    assert_eq!(get_block(db, ctx.clone(), data2.clone()), Ok(Some(data2)));

    match block_on(db.insert_batch(ctx, DataCategory::Block, vec![b"test3".to_vec()], vec![])) {
        Err(DatabaseError::InvalidData) => (), // pass
        _ => panic!("should return error DatabaseError::InvalidData"),
    }
}

pub fn test_contains<D: Database>(db: &D) {
    let ctx = Context::new();
    let data = b"test".to_vec();
    let none_exist = b"none_exist".to_vec();

    block_on(db.insert(ctx.clone(), DataCategory::Block, data.clone(), data.clone())).unwrap();

    assert_eq!(
        block_on(db.contains(ctx.clone(), DataCategory::Block, &data)),
        Ok(true)
    );

    assert_eq!(
        block_on(db.contains(ctx, DataCategory::Block, &none_exist)),
        Ok(false)
    );
}

pub fn test_remove<D: Database>(db: &D) {
    let ctx = Context::new();
    let data = b"test".to_vec();

    block_on(db.insert(ctx.clone(), DataCategory::Block, data.clone(), data.clone())).unwrap();

    block_on(db.remove(ctx.clone(), DataCategory::Block, &data)).unwrap();
    assert_eq!(get_block(db, ctx, data), Ok(None));
}

pub fn test_remove_batch<D: Database>(db: &D) {
    let ctx = Context::new();
    let data1 = b"test1".to_vec();
    let data2 = b"test2".to_vec();

    block_on(db.insert_batch(
        ctx.clone(),
        DataCategory::Block,
        vec![data1.clone(), data2.clone()],
        vec![data1.clone(), data2.clone()],
    ))
    .unwrap();

    block_on(db.remove_batch(ctx.clone(), DataCategory::Block, &[
        data1.clone(),
        data2.clone(),
    ]))
    .unwrap();

    assert_eq!(get_block(db, ctx.clone(), data1), Ok(None));
    assert_eq!(get_block(db, ctx, data2), Ok(None));
}
