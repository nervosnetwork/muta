use std::{
    cmp::PartialEq,
    convert::TryFrom,
    hash::{Hash, Hasher},
    str::FromStr,
};

use derive_more::{Display, From};

use crate::error::{ErrorKind, NetworkError};

pub const GOSSIP_SCHEME: &str = "/gossip";
pub const RPC_CALL_SCHEME: &str = "/rpc_call";
pub const RPC_RESPONSE_SCHEME: &str = "/rpc_resp";

pub const MAX_ENDPOINT_LENGTH: usize = 120;

#[derive(Debug, Display, PartialEq, Eq)]
pub enum EndpointScheme {
    #[display(fmt = "{}", GOSSIP_SCHEME)]
    Gossip,

    #[display(fmt = "{}", RPC_CALL_SCHEME)]
    RpcCall,

    #[display(fmt = "{}", RPC_RESPONSE_SCHEME)]
    RpcResponse,
}

// For example
//
// gossip: /gossip/cprd/7702_cnpukpeyr_release_date
// rpc: /rpc_call/cykppeunr_7702/create_a_character/{rpc_id}
//
// NOTE: Endpoint only care about first three url comps. So
// as its PartialEq, Eq and Hash implement.
#[derive(Debug, Clone, Display)]
#[display(fmt = "{}", _0)]
pub struct Endpoint(String);

impl Endpoint {
    pub fn starts_with(&self, pat: &str) -> bool {
        self.0.starts_with(pat)
    }

    pub fn scheme(&self) -> EndpointScheme {
        if self.starts_with(GOSSIP_SCHEME) {
            EndpointScheme::Gossip
        } else if self.starts_with(RPC_CALL_SCHEME) {
            EndpointScheme::RpcCall
        } else if self.starts_with(RPC_RESPONSE_SCHEME) {
            EndpointScheme::RpcResponse
        } else {
            unreachable!()
        }
    }

    // Root part, the first three comps
    pub fn root(&self) -> String {
        let url = &self.0;

        let comps = url
            .split('/')
            .filter(|comp| !comp.is_empty())
            .collect::<Vec<&str>>();

        format!("/{}/{}/{}", comps[0], comps[1], comps[2])
    }

    pub fn full_url(&self) -> &str {
        &self.0
    }

    pub fn extend(&self, comp: &str) -> Result<Self, NetworkError> {
        let comp = comp.trim_start_matches('/');

        format!("{}/{}", self.0, comp).parse::<Endpoint>()
    }
}

impl PartialEq for Endpoint {
    fn eq(&self, other: &Self) -> bool {
        self.root() == other.root()
    }
}

impl Eq for Endpoint {}

impl Hash for Endpoint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.root().hash(state)
    }
}

impl FromStr for Endpoint {
    type Err = NetworkError;

    fn from_str(end: &str) -> Result<Self, Self::Err> {
        if end.is_empty() || end.len() > MAX_ENDPOINT_LENGTH {
            return Err(NetworkError::NotEndpoint);
        }

        // Check scheme
        if !end.starts_with(GOSSIP_SCHEME)
            && !end.starts_with(RPC_CALL_SCHEME)
            && !end.starts_with(RPC_RESPONSE_SCHEME)
        {
            return Err(NetworkError::UnexpectedScheme(end.to_owned()));
        }

        // Count components
        let comps = end
            .split('/')
            .filter(|comp| !comp.is_empty())
            .collect::<Vec<&str>>();

        // Right now, gossip takes 3 comps and rpc has 4 comps
        if comps.len() < 3 || comps.len() > 4 {
            return Err(NetworkError::NotEndpoint);
        }

        Ok(Endpoint(end.to_owned()))
    }
}

#[derive(Debug, PartialEq, Eq, From, Display, Hash, Clone, Copy)]
#[display(fmt = "{}", _0)]
pub struct RpcId(u64);

impl RpcId {
    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, From, Display)]
#[display(fmt = "{}/{}", end, rid)]
pub struct RpcEndpoint {
    end: Endpoint,
    rid: RpcId,
}

impl RpcEndpoint {
    pub fn endpoint(&self) -> &Endpoint {
        &self.end
    }

    pub fn rpc_id(&self) -> RpcId {
        self.rid
    }

    fn extract_rpc_id_from(end: &Endpoint) -> Result<RpcId, NetworkError> {
        let end = end.full_url();

        // Rpc id should be the last comp
        let r_sep_idx = end.rfind('/').ok_or(NetworkError::NotEndpoint)?;
        if end.len() == (r_sep_idx + 1) {
            // Last separator '/' should not be the last char
            return Err(NetworkError::NotEndpoint);
        }

        // Extract rid
        let rid = &end[(r_sep_idx + 1)..];

        // Parse it
        let rid = rid.parse::<u64>().map_err(ErrorKind::NotIdString)?;

        Ok(rid.into())
    }
}

impl TryFrom<Endpoint> for RpcEndpoint {
    type Error = NetworkError;

    fn try_from(end: Endpoint) -> Result<Self, Self::Error> {
        let rid = Self::extract_rpc_id_from(&end)?;

        Ok(RpcEndpoint { end, rid })
    }
}

impl FromStr for RpcEndpoint {
    type Err = NetworkError;

    fn from_str(end: &str) -> Result<Self, Self::Err> {
        let end = end.parse::<Endpoint>()?;

        if !end.starts_with(RPC_CALL_SCHEME) && !end.starts_with(RPC_RESPONSE_SCHEME) {
            return Err(NetworkError::UnexpectedScheme(end.root()));
        }

        let rid = Self::extract_rpc_id_from(&end)?;

        Ok(RpcEndpoint { end, rid })
    }
}

#[cfg(test)]
mod tests {
    use super::Endpoint;

    #[test]
    fn should_able_parse_valid_endpoint_url() {
        let end = "/gossip/crpd/watch_cpunpyker7702";
        let expect = Endpoint(end.to_owned());

        let endpoint = end.parse::<Endpoint>().unwrap();
        assert_eq!(endpoint, expect);
    }
}
