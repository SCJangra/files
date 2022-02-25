use jsonrpc_core as jrpc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Progress {
    pub total: u64,
    pub done: u64,
    pub percent: f64,
}

pub type JrpcFutResult<T> = jrpc::BoxFuture<jrpc::Result<T>>;
