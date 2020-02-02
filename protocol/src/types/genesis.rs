use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Genesis {
    pub timestamp: u64,
    pub prevhash:  String,
    pub services:  Vec<ServiceParam>,
}

impl Genesis {
    pub fn get_payload(&self, name: &str) -> &str {
        &self
            .services
            .iter()
            .find(|&service| service.name == name)
            .expect("miss metadata service!")
            .payload
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ServiceParam {
    pub name:    String,
    pub payload: String,
}
