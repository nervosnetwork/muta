use prost::Enumeration;

use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
pub enum Method {
    BroadcastTxs = 0,
    PullTxs = 1,
    PushTxs = 2,
    Proposal = 3,
    Vote = 4,
    SyncBroadcastStatus = 5,
    SyncPullBlocks = 6,
    SyncPushBlocks = 7,
    SyncPullTxs = 8,
    SyncPushTxs = 9,
}

macro_rules! impl_from_to {
    ($from_ident:ident, $to_ident:ident, $ty:ty) => {
        impl Method {
            pub fn $from_ident(value: $ty) -> Result<Self, Error> {
                match value {
                    0 => Ok(Method::BroadcastTxs),
                    1 => Ok(Method::PullTxs),
                    2 => Ok(Method::PushTxs),
                    3 => Ok(Method::Proposal),
                    4 => Ok(Method::Vote),
                    5 => Ok(Method::SyncBroadcastStatus),
                    6 => Ok(Method::SyncPullBlocks),
                    7 => Ok(Method::SyncPushBlocks),
                    8 => Ok(Method::SyncPullTxs),
                    9 => Ok(Method::SyncPushTxs),
                    _ => Err(Error::UnknownMethod(u32::from(value))),
                }
            }

            pub fn $to_ident(&self) -> $ty {
                match self {
                    Method::BroadcastTxs => 0,
                    Method::PullTxs => 1,
                    Method::PushTxs => 2,
                    Method::Proposal => 3,
                    Method::Vote => 4,
                    Method::SyncBroadcastStatus => 5,
                    Method::SyncPullBlocks => 6,
                    Method::SyncPushBlocks => 7,
                    Method::SyncPullTxs => 8,
                    Method::SyncPushTxs => 9,
                }
            }
        }
    };
}

// note: prost implement from_i32 for us, and use `as i32` to convert to i32
impl_from_to!(from_u8, to_u8, u8);
impl_from_to!(from_u32, to_u32, u32);
