use std::convert::Into;
use std::marker::Send;

use futures::future::Future;
use grpc::SingleResponse;

use crate::{
    error::ServiceErrorExt,
    service::{error::ServiceError, FutResponse},
};

pub trait SingleResponseExt<T> {
    fn into_fut_resp(self) -> FutResponse<T>;
}

pub trait FutResponseExt<T: Send + 'static> {
    fn into_single_resp(self) -> SingleResponse<T>;
}

pub struct RpcFutResponse<T: Send + 'static>(pub FutResponse<T>);

impl<T: Send + 'static> From<SingleResponse<T>> for RpcFutResponse<T> {
    fn from(resp: SingleResponse<T>) -> Self {
        RpcFutResponse(Box::new(
            resp.drop_metadata().map_err(ServiceError::from_grpc_err),
        ))
    }
}

impl<T: Send + 'static> Into<SingleResponse<T>> for RpcFutResponse<T> {
    fn into(self) -> SingleResponse<T> {
        SingleResponse::no_metadata(self.0.map_err(ServiceError::into_grpc_err))
    }
}

impl<T: Send + 'static> SingleResponseExt<T> for SingleResponse<T> {
    fn into_fut_resp(self) -> FutResponse<T> {
        RpcFutResponse::from(self).0
    }
}

impl<T: Send + 'static> FutResponseExt<T> for FutResponse<T> {
    fn into_single_resp(self) -> SingleResponse<T> {
        RpcFutResponse(self).into()
    }
}
