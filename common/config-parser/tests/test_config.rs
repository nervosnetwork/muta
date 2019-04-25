use config_parser::{parse, parse_file, parse_http, parse_reader};
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
