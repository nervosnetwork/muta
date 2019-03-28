use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ParseError {
    IO(io::Error),
    Deserialize(toml::de::Error),
    Reqwest(reqwest::Error),
}

impl error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::IO(e) => return write!(f, "{}", e),
            ParseError::Deserialize(e) => return write!(f, "{}", e),
            ParseError::Reqwest(e) => return write!(f, "{}", e),
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(error: io::Error) -> ParseError {
        ParseError::IO(error)
    }
}

impl From<toml::de::Error> for ParseError {
    fn from(error: toml::de::Error) -> ParseError {
        ParseError::Deserialize(error)
    }
}

impl From<reqwest::Error> for ParseError {
    fn from(error: reqwest::Error) -> ParseError {
        ParseError::Reqwest(error)
    }
}
