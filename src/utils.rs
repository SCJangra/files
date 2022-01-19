use jsonrpc_core::{self as jrpc, serde_json as json};

pub fn to_rpc_err(err: anyhow::Error) -> jrpc::Error {
    jrpc::Error {
        code: jrpc::ErrorCode::ServerError(-32001),
        message: err.to_string(),
        data: Some(json::json!(err.root_cause().to_string())),
    }
}
