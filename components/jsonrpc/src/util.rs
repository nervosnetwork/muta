use crate::error::RpcError;
use crate::RpcResult;

pub fn clean_0x(s: &str) -> &str {
    if s.starts_with("0x") {
        &s[2..]
    } else {
        s
    }
}

pub fn u64_from_string(s: &str) -> RpcResult<u64> {
    if s.starts_with("0x") {
        let s = clean_0x(s);
        Ok(u64::from_str_radix(s, 16).map_err(|e| RpcError::Str(format!("{:?}", e)))?)
    } else {
        Ok(u64::from_str_radix(s, 10).map_err(|e| RpcError::Str(format!("{:?}", e)))?)
    }
}
