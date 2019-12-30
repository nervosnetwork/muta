use serde_derive::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Genesis {
    pub timestamp:    u64,
    pub prevhash:     String,
    pub system_token: GenesisSystemToken,
    pub state_alloc:  Vec<GenesisStateAlloc>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GenesisStateAlloc {
    pub address: String,
    pub assets:  Vec<GenesisStateAsset>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GenesisStateAsset {
    pub asset_id: String,
    pub balance:  String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GenesisSystemToken {
    pub code:   String,
    pub name:   String,
    pub symbol: String,
    pub supply: u64,
}

#[cfg(test)]
mod tests {
    use super::Genesis;

    #[test]
    fn test_name() {
        let genesis_string = r#"{
            "timestamp": 100000,
            "prevhash": "0x0000000000",
            "system_token": {
                "code": "",
                "name": "Muta system token",
                "symbol": "MST",
                "supply": 21000000000
            },
            "state_alloc": [
                {
                    "address": "0xfffff",
                    "assets": [{
                        "asset_id": "0xff",
                        "balance": "0xfff"
                    }]
                }
            ]
        }"#;

        let _: Genesis = serde_json::from_str(genesis_string).unwrap();
    }
}
