use crate::FutRuntimeResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseError {
    NotFound,
    InvalidData,
    Internal,
}

pub trait DatabaseFactory: Send + Sync {
    type Instance: DatabaseInstance;

    fn crate_instance(&self) -> FutRuntimeResult<Self::Instance, DatabaseError>;
}

pub trait DatabaseInstance {
    fn get(&self, key: &[u8]) -> FutRuntimeResult<Vec<u8>, DatabaseError>;

    fn get_batch(&self, keys: &[Vec<u8>]) -> FutRuntimeResult<Vec<Option<Vec<u8>>>, DatabaseError>;

    fn insert(&mut self, key: &[u8], value: &[u8]) -> FutRuntimeResult<(), DatabaseError>;

    fn insert_batch(
        &mut self,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
    ) -> FutRuntimeResult<(), DatabaseError>;

    fn contain(&self, key: &[u8]) -> FutRuntimeResult<bool, DatabaseError>;

    fn remove(&mut self, key: &[u8]) -> FutRuntimeResult<(), DatabaseError>;

    fn remove_batch(&mut self, keys: &[Vec<u8>]) -> FutRuntimeResult<(), DatabaseError>;
}
