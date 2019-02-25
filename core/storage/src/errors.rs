use failure::Fail;

#[derive(Debug, Fail)]
pub enum StorageError {
    #[fail(display = "not found")]
    NotFound,
}
