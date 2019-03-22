//! Crate config gives some easy way to parse a config file from Reader,
//! File or HTTP Server.
use serde::de;
use std::fs;
use std::io;
use std::path::Path;
mod err;
pub use err::ConfigError;

/// Parse a config from reader.
pub fn from_reader<R: io::Read, T: de::DeserializeOwned>(r: &mut R) -> Result<T, ConfigError> {
    let mut buf = Vec::new();
    r.read_to_end(&mut buf)?;
    Ok(toml::from_slice(&buf)?)
}

/// Parse a config from file.
///
/// Note: In most cases, function `from` is better.
pub fn from_file<T: de::DeserializeOwned>(name: impl AsRef<Path>) -> Result<T, ConfigError> {
    let mut f = fs::File::open(name)?;
    from_reader(&mut f)
}

/// Parse a config from method of HTTP GET.
///
/// Note: In most cases, function `from` is better.
pub fn from_http<T: de::DeserializeOwned>(name: &str) -> Result<T, ConfigError> {
    let mut r = reqwest::get(name)?;
    from_reader(&mut r)
}

/// If name is starts with "http", parse it by function `from_http`, else
/// `from_file` in use.
pub fn from<T: de::DeserializeOwned>(name: &str) -> Result<T, ConfigError> {
    if name.starts_with("http") {
        from_http(name)
    } else {
        from_file(name)
    }
}
