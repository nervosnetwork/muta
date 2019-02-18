use grpc::Error as GrpcError;

use crate::service::error::ServiceError;

pub trait ServiceErrorExt {
    fn into_grpc_err(self) -> GrpcError;
    fn from_grpc_err(err: GrpcError) -> ServiceError;
}

impl ServiceErrorExt for ServiceError {
    fn into_grpc_err(self) -> GrpcError {
        use futures::sync::oneshot::Canceled;
        use grpc::GrpcMessageError;

        match self {
            ServiceError::Io(err) => GrpcError::Io(err),
            ServiceError::RpcMessage { status, message } => {
                GrpcError::GrpcMessage(GrpcMessageError {
                    grpc_status: status,
                    grpc_message: message,
                })
            }
            ServiceError::Canceled(_err) => GrpcError::Canceled(Canceled {}),
            ServiceError::Panic(err_string) => GrpcError::Panic(err_string),
            ServiceError::Other(err_str) => GrpcError::Other(err_str),
        }
    }

    // TODO: remove 65534, 65533, 65532 error mapping
    fn from_grpc_err(err: GrpcError) -> ServiceError {
        use crate::service::error::Canceled;

        match err {
            GrpcError::Io(err) => ServiceError::Io(err),
            GrpcError::Http(err) => ServiceError::RpcMessage {
                status: 65534,
                message: err.to_string(),
            },
            GrpcError::GrpcMessage(msg_err) => ServiceError::RpcMessage {
                status: msg_err.grpc_status,
                message: msg_err.grpc_message,
            },
            GrpcError::Canceled(_err) => ServiceError::Canceled(Canceled {}),
            GrpcError::MetadataDecode(_) => ServiceError::RpcMessage {
                status: 65533,
                message: String::from("rpc inner base64 decode error"),
            },
            GrpcError::Protobuf(proto_err) => ServiceError::RpcMessage {
                status: 65532,
                message: proto_err.to_string(),
            },
            GrpcError::Panic(err_string) => ServiceError::Panic(err_string),
            GrpcError::Other(err_str) => ServiceError::Other(err_str),
        }
    }
}
