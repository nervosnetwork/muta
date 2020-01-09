use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Genesis {
    pub timestamp: u64,
    pub prevhash:  String,
    pub services:  Vec<ServiceParam>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ServiceParam {
    pub name:    String,
    pub payload: String,
}
