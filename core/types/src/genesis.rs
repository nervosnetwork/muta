use std::collections::HashMap;

use serde_derive::Deserialize;

// TODO: refactor to
// pub struct Genesis {
//     pub timestamp: u64,
//     pub prevhash: Hash,
//     pub state_alloc: Vec<StateAlloc>,
// }
#[derive(Default, Clone, Debug, Deserialize)]
pub struct Genesis {
    pub timestamp: u64,
    pub prevhash: String,
    pub state_alloc: Vec<StateAlloc>,
}

// TODO: refactor to
// pub struct StateAlloc {
//     pub address: Address,
//     pub code: Vec<u8>,
//     pub storage: HashMap<Vec<u8>, Vec<u8>>,
//     pub balance: Balance,
// }
#[derive(Default, Clone, Debug, Deserialize)]
pub struct StateAlloc {
    pub address: String,
    pub code: String,
    pub storage: HashMap<String, String>,
    pub balance: String,
}

#[cfg(test)]
mod tests {
    use super::Genesis;

    #[test]
    fn test_name() {
        let genesis_string = r#"{
            "timestamp": 100000,
            "prevhash": "0x0000000000",
            "state_alloc": [
                {
                    "address": "0xfffff",
                    "code": "0xfffff",
                    "storage": {
                        "0xf": "0xff",
                        "0xff": "0xfff"
                    },
                    "balance": "0xfff"
                }
            ]
        }"#;

        let _: Genesis = serde_json::from_str(genesis_string).unwrap();
    }
}
