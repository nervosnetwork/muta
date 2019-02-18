use failure::Fail;

#[derive(Debug)]
pub struct Canceled;

#[derive(Debug, Fail)]
pub enum ServiceError {
    #[fail(display = "{}", _0)]
    Io(#[fail(cause)] std::io::Error),

    #[fail(display = "status: {}, messsage: {}", status, message)]
    RpcMessage { status: i32, message: String },

    #[fail(display = "canceled")]
    Canceled(Canceled),

    #[fail(display = "panic: {}", _0)]
    Panic(String),

    #[fail(display = "other: {}", _0)]
    Other(&'static str),
}
