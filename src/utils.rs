use jsonrpc_core::{self as jrpc, serde_json as json};

pub fn to_rpc_err(err: anyhow::Error) -> jrpc::Error {
    jrpc::Error {
        code: jrpc::ErrorCode::ServerError(-32001),
        message: err.to_string(),
        data: Some(json::json!(err.root_cause().to_string())),
    }
}

#[macro_export]
macro_rules! notify_err {
    ($sink: ident, $err: expr) => {{
        $sink
            .notify(Err($err))
            .map_err(|e| anyhow::anyhow!("Could not notify error '{}'", e))
    }};
}

#[macro_export]
macro_rules! notify_ok {
    ($sink: ident, $val: expr) => {{
        $sink
            .notify(Ok($val))
            .map_err(|e| anyhow::anyhow!("Could not notify result '{}'", e))
    }};
}
