#[macro_export]
macro_rules! callback_broadcast {
    ($outbound:expr, $ctx:expr, $arg:expr, $data_ty:ty, $method:expr, $err_ty:expr) => {{
        use futures::channel::mpsc::channel;
        use futures::prelude::{FutureExt, StreamExt};

        use crate::common::scope_from_context;

        let s_method = stringify!($method);

        let fut = async move {
            let scope = scope_from_context($ctx).ok_or($err_ty(format!(
                "net [outbound]: {}: session id not found",
                s_method
            )))?;

            let uid = $outbound.callback.new_uid();
            let data = <$data_ty>::from(uid, $arg);

            // TODO: retry?
            $outbound
                .quick_filter_broadcast($method, data, scope)
                .map_err(|err| {
                    $err_ty(format!("net [outbound]: {}, [err: {:?}]", s_method, err))
                })?;

            let (done_tx, mut done_rx) = channel(1);
            $outbound.callback.insert(uid, done_tx);

            // TODO: Timeout
            await!(done_rx.next()).ok_or($err_ty(format!(
                "net [outbound]: {}: done_rx return None",
                s_method
            )))
        };

        Box::new(fut.boxed().compat())
    }};
}
