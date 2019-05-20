use prost::Message as ProstMessage;

#[derive(Clone, PartialEq, ProstMessage)]
pub struct Proposal {
    #[prost(bytes, tag = "1")]
    pub data: Vec<u8>,
}

impl Proposal {
    pub fn from(data: Vec<u8>) -> Self {
        Proposal { data }
    }

    pub fn des(self) -> Vec<u8> {
        self.data
    }
}

#[derive(Clone, PartialEq, ProstMessage)]
pub struct Vote {
    #[prost(bytes, tag = "1")]
    pub data: Vec<u8>,
}

impl Vote {
    pub fn from(data: Vec<u8>) -> Self {
        Vote { data }
    }

    pub fn des(self) -> Vec<u8> {
        self.data
    }
}
