use derive_more::{Display, From};
use protocol::{ProtocolError, ProtocolErrorKind};

#[derive(Debug, Display, From)]
pub enum MainError {
    #[display(fmt = "The muta configuration read failed {:?}", _0)]
    ConfigParse(common_config_parser::ParseError),

    #[display(fmt = "{:?}", _0)]
    Io(std::io::Error),

    #[display(fmt = "Toml fails to parse genesis {:?}", _0)]
    GenesisTomlDe(toml::de::Error),

    #[display(fmt = "hex error {:?}", _0)]
    FromHex(hex::FromHexError),

    #[display(fmt = "crypto error {:?}", _0)]
    Crypto(common_crypto::Error),

    #[display(fmt = "{:?}", _0)]
    Utf8(std::str::Utf8Error),

    #[display(fmt = "other error {:?}", _0)]
    Other(String),
}

impl std::error::Error for MainError {}

impl From<MainError> for ProtocolError {
    fn from(error: MainError) -> ProtocolError {
        ProtocolError::new(ProtocolErrorKind::Main, Box::new(error))
    }
}
