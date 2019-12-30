use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Genesis {
    pub timestamp: u64,
    pub prevhash:  String,
    pub services:  Vec<GenesisService>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GenesisService {
    pub caller:  String,
    pub service: String,
    pub method:  String,
    pub payload: String,
}
