use num_cpus;
use serde_derive::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Config {
    pub listen_address: String,
    pub threads: usize,
    pub max_request_body_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_address: "127.0.0.1:3030".to_string(),
            max_request_body_size: 10_485_760, // 10Mib
            threads: num_cpus::get(),
        }
    }
}
