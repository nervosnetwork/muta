use futures::future::Future;

use crate::error::ServiceError;

pub type FutResponse<T> = Box<dyn Future<Item = T, Error = ServiceError> + Send>;
