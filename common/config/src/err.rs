use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum ConfigError {
    IO(io::Error),
    Deserialize(toml::de::Error),
    Reqwest(reqwest::Error),
}

impl error::Error for ConfigError {}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::IO(e) => return write!(f, "{}", e),
            ConfigError::Deserialize(e) => return write!(f, "{}", e),
            ConfigError::Reqwest(e) => return write!(f, "{}", e),
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(error: io::Error) -> ConfigError {
        ConfigError::IO(error)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(error: toml::de::Error) -> ConfigError {
        ConfigError::Deserialize(error)
    }
}

impl From<reqwest::Error> for ConfigError {
    fn from(error: reqwest::Error) -> ConfigError {
        ConfigError::Reqwest(error)
    }
}
