use serde::de;

use std::error;
use std::fmt;
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

#[cfg(test)]
mod tests {
    use super::{parse, parse_file, parse_http, parse_reader};
    use serde_derive::Deserialize;
    use stringreader::StringReader;

    #[derive(Debug, Deserialize)]
    struct Config {
        global_string: Option<String>,
        global_int:    Option<u64>,
    }

    #[test]
    fn test_parse_reader() {
        let toml_str = r#"
        global_string = "Best Food"
        global_int = 42
    "#;
        let mut toml_r = StringReader::new(toml_str);
        let config: Config = parse_reader(&mut toml_r).unwrap();
        assert_eq!(config.global_string, Some(String::from("Best Food")));
        assert_eq!(config.global_int, Some(42));
    }

    #[ignore]
    #[test]
    fn test_parse_file() {
        let config: Config = parse_file("/tmp/config.toml").unwrap();
        assert_eq!(config.global_string, Some(String::from("Best Food")));
        assert_eq!(config.global_int, Some(42));
    }

    #[ignore]
    #[test]
    fn test_parse_http() {
        let config: Config = parse_http("http://127.0.0.1:8080/config.toml").unwrap();
        assert_eq!(config.global_string, Some(String::from("Best Food")));
        assert_eq!(config.global_int, Some(42));
    }

    #[ignore]
    #[test]
    fn test_parse() {
        let config: Config = parse("http://127.0.0.1:8080/config.toml").unwrap();
        assert_eq!(config.global_string, Some(String::from("Best Food")));
        assert_eq!(config.global_int, Some(42));
        let config: Config = parse("/tmp/config.toml").unwrap();
        assert_eq!(config.global_string, Some(String::from("Best Food")));
        assert_eq!(config.global_int, Some(42));
    }
}
