use serde::Deserialize;

use crate::types::primitive::Hex;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Genesis {
    pub timestamp: u64,
    pub prevhash:  Hex,
    pub services:  Vec<ServiceParam>,
}

impl Genesis {
    pub fn get_payload(&self, name: &str) -> &str {
        &self
            .services
            .iter()
            .find(|&service| service.name == name)
            .unwrap_or_else(|| panic!("miss {:?} service!", name))
            .payload
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ServiceParam {
    pub name:    String,
    pub payload: String,
}
