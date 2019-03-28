//! Crate config gives some easy way to parse a config file from Reader,
//! File or HTTP Server.
mod err;
pub use err::ParseError;

use serde::de;

use std::fs;
use std::io;
use std::path::Path;

/// Parse a config from reader.
pub fn parse_reader<R: io::Read, T: de::DeserializeOwned>(r: &mut R) -> Result<T, ParseError> {
    let mut buf = Vec::new();
    r.read_to_end(&mut buf)?;
    Ok(toml::from_slice(&buf)?)
}

/// Parse a config from file.
///
/// Note: In most cases, function `parse` is better.
pub fn parse_file<T: de::DeserializeOwned>(name: impl AsRef<Path>) -> Result<T, ParseError> {
    let mut f = fs::File::open(name)?;
    parse_reader(&mut f)
}

// FIXME: http is inscure, support https only
/// Parse a config from method of HTTP GET.
///
/// Note: In most cases, function `parse` is better.
pub fn parse_http<T: de::DeserializeOwned>(name: &str) -> Result<T, ParseError> {
    let mut r = reqwest::get(name)?;
    parse_reader(&mut r)
}

/// If name is starts with "http", parse it by function `parse_http`, else
/// `parse_file` in use.
pub fn parse<T: de::DeserializeOwned>(name: &str) -> Result<T, ParseError> {
    if name.starts_with("http") {
        parse_http(name)
    } else {
        parse_file(name)
    }
}
