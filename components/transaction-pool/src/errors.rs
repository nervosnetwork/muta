use failure::Fail;

#[derive(Debug, Fail)]
pub enum OrderError {
    #[fail(display = "reach the limit")]
    ReachLimit,
}

#[derive(Debug, Fail)]
pub enum VerifierError {
    #[fail(display = "signature invalid")]
    SignatureInvalid,
}

#[derive(Debug, Fail)]
pub enum TransactionPoolError {
    #[fail(display = "signature invalid")]
    SignatureInvalid,
}
