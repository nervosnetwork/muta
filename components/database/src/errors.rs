use failure::Fail;

#[derive(Debug, Fail)]
pub enum MemoryDBError {
    #[fail(display = "not found")]
    NotFound,
}
