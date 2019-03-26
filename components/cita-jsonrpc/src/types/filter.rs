use core_types::{Address, Bloom, BloomInput, Hash, LogEntry};
use jsonrpc_types::rpctypes::{BlockNumber, Filter as RpcFilter, VariadicValue};

#[derive(Debug, Clone)]
pub struct Filter {
    pub from_block: BlockNumber,
    pub to_block: BlockNumber,
    pub address: Option<Vec<Address>>,
    pub topics: Vec<Option<Vec<Hash>>>,
    pub limit: Option<usize>,
}

impl Filter {
    pub fn bloom_possibilities(&self) -> Vec<Bloom> {
        let blooms = match self.address {
            Some(ref addresses) if !addresses.is_empty() => addresses
                .iter()
                .map(|ref address| Bloom::from(BloomInput::Raw(address.as_ref())))
                .collect(),
            _ => vec![Bloom::default()],
        };

        self.topics.iter().fold(blooms, |bs, topic| match *topic {
            None => bs,
            Some(ref topics) => bs
                .into_iter()
                .flat_map(|bloom| {
                    topics
                        .iter()
                        .map(|topic| {
                            let mut b = bloom;
                            b.accrue(BloomInput::Raw(topic.as_ref()));
                            b
                        })
                        .collect::<Vec<Bloom>>()
                })
                .collect(),
        })
    }

    pub fn matches(&self, log: &LogEntry) -> bool {
        let matches = match self.address {
            Some(ref addresses) if !addresses.is_empty() => {
                addresses.iter().any(|address| &log.address == address)
            }
            _ => true,
        };

        matches
            && self
                .topics
                .iter()
                .enumerate()
                .all(|(i, topic)| match *topic {
                    Some(ref topics) if !topics.is_empty() => {
                        topics.iter().any(|topic| log.topics.get(i) == Some(topic))
                    }
                    _ => true,
                })
    }
}

impl From<RpcFilter> for Filter {
    fn from(v: RpcFilter) -> Filter {
        Filter {
            from_block: v.from_block,
            to_block: v.to_block,
            address: v.address.and_then(|address| match address {
                VariadicValue::Null => None,
                VariadicValue::Single(a) => {
                    Some(vec![Address::from(Into::<Vec<u8>>::into(a).as_slice())])
                }
                VariadicValue::Multiple(a) => Some(
                    a.into_iter()
                        .map(|addr| Address::from(Into::<Vec<u8>>::into(addr).as_slice()))
                        .collect(),
                ),
            }),
            topics: {
                let mut iter = v
                    .topics
                    .map_or_else(Vec::new, |topics| {
                        topics
                            .into_iter()
                            .take(4)
                            .map(|topic| match topic {
                                VariadicValue::Null => None,
                                VariadicValue::Single(a) => {
                                    Some(vec![Hash::from_raw(&Into::<Vec<u8>>::into(a))])
                                }
                                VariadicValue::Multiple(a) => Some(
                                    a.into_iter()
                                        .map(|h| Hash::from_raw(&Into::<Vec<u8>>::into(h)))
                                        .collect(),
                                ),
                            })
                            .collect()
                    })
                    .into_iter();

                vec![
                    iter.next().unwrap_or(None),
                    iter.next().unwrap_or(None),
                    iter.next().unwrap_or(None),
                    iter.next().unwrap_or(None),
                ]
            },
            limit: v.limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Filter;
    use core_types::{Address, Bloom, Hash, LogEntry};
    use jsonrpc_types::rpctypes::BlockNumber;
    use std::str::FromStr;

    #[test]
    fn test_bloom_possibilities_none() {
        let none_filter = Filter {
            from_block: BlockNumber::default(),
            to_block: BlockNumber::default(),
            address: None,
            topics: vec![None, None, None, None],
            limit: None,
        };

        let possibilities = none_filter.bloom_possibilities();
        assert_eq!(possibilities.len(), 1);
        assert!(possibilities[0].is_empty())
    }

    // block 399849
    #[test]
    fn test_bloom_possibilities_single_address_and_topic() {
        let filter = Filter {
            from_block: BlockNumber::default(),
            to_block: BlockNumber::default(),
            address: Some(vec![Address::from_hex(
                "b372018f3be9e171df0581136b59d2faf73a7d5d",
            )
            .unwrap()]),
            topics: vec![
                Some(vec![Hash::from_hex(
                    "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
                )
                .unwrap()]),
                None,
                None,
                None,
            ],
            limit: None,
        };
        let possibilities = filter.bloom_possibilities();
        let blooms: Vec<Bloom> = vec![Bloom::from_str(
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap()];
        assert_eq!(possibilities, blooms);
    }

    #[test]
    fn test_bloom_possibilities_single_address_and_many_topics() {
        let filter = Filter {
            from_block: BlockNumber::default(),
            to_block: BlockNumber::default(),
            address: Some(vec![Address::from_hex(
                "b372018f3be9e171df0581136b59d2faf73a7d5d",
            )
            .unwrap()]),
            topics: vec![
                Some(vec![Hash::from_hex(
                    "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
                )
                .unwrap()]),
                Some(vec![Hash::from_hex(
                    "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
                )
                .unwrap()]),
                None,
                None,
            ],
            limit: None,
        };
        let possibilities = filter.bloom_possibilities();
        let blooms: Vec<Bloom> = vec![Bloom::from_str(
            "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap()];
        assert_eq!(possibilities, blooms);
    }

    #[test]
    fn test_bloom_possibilites_multiple_addresses_and_topics() {
        let filter = Filter {
            from_block: BlockNumber::default(),
            to_block: BlockNumber::default(),
            address: Some(vec![
                Address::from_hex("b372018f3be9e171df0581136b59d2faf73a7d5d").unwrap(),
                Address::from_hex("b372018f3be9e171df0581136b59d2faf73a7d5d").unwrap(),
            ]),
            topics: vec![
                Some(vec![
                    Hash::from_hex(
                        "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
                    )
                    .unwrap(),
                    Hash::from_hex(
                        "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
                    )
                    .unwrap(),
                ]),
                Some(vec![
                    Hash::from_hex(
                        "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
                    )
                    .unwrap(),
                    Hash::from_hex(
                        "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
                    )
                    .unwrap(),
                ]),
                Some(vec![Hash::from_hex(
                    "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
                )
                .unwrap()]),
                None,
            ],
            limit: None,
        };

        // number of possibilites should be equal 2 * 2 * 2 * 1 = 8
        let possibilities = filter.bloom_possibilities();
        let blooms: Vec<Bloom> = vec![
            Bloom::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            Bloom::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            Bloom::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            Bloom::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            Bloom::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            Bloom::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            Bloom::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            Bloom::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000008
             0000000000000000000000000000000000000000800000000000000000000000
             0000000000000000000000000000000000000000000000400000000400000000
             0000000000000000000000000000000000000000020000000000000000000000
             0000000000000000000000000000000000000000000000000000000000000000
             0000000000000000000000000000000000000000000000000000040000000000
             0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
        ];
        assert_eq!(possibilities, blooms);
    }

    #[test]
    fn test_filter_matches() {
        let filter = Filter {
            from_block: BlockNumber::default(),
            to_block: BlockNumber::default(),
            address: Some(vec![Address::from_hex(
                "b372018f3be9e171df0581136b59d2faf73a7d5d",
            )
            .unwrap()]),
            topics: vec![
                Some(vec![Hash::from_hex(
                    "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
                )
                .unwrap()]),
                Some(vec![Hash::from_hex(
                    "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23fa",
                )
                .unwrap()]),
                None,
                None,
            ],
            limit: None,
        };

        let entry0 = LogEntry {
            address: Address::from_hex("b372018f3be9e171df0581136b59d2faf73a7d5d").unwrap(),
            topics: vec![
                Hash::from_hex("ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9")
                    .unwrap(),
                Hash::from_hex("ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23fa")
                    .unwrap(),
                Hash::from_hex("ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9")
                    .unwrap(),
            ],
            data: vec![],
        };

        let entry1 = LogEntry {
            address: Address::from_hex("b372018f3be9e171df0581136b59d2faf73a7d5e").unwrap(),
            topics: vec![
                Hash::from_hex("ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9")
                    .unwrap(),
                Hash::from_hex("ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23fa")
                    .unwrap(),
                Hash::from_hex("ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9")
                    .unwrap(),
            ],
            data: vec![],
        };

        let entry2 = LogEntry {
            address: Address::from_hex("b372018f3be9e171df0581136b59d2faf73a7d5d").unwrap(),
            topics: vec![Hash::from_hex(
                "ff74e91598aed6ae5d2fdcf8b24cd2c7be49a0808112a305069355b7160f23f9",
            )
            .unwrap()],
            data: vec![],
        };

        assert_eq!(filter.matches(&entry0), true);
        assert_eq!(filter.matches(&entry1), false);
        assert_eq!(filter.matches(&entry2), false);
    }
}
