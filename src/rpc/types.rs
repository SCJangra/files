use jsonrpc_core as jrpc;
use serde::{Deserialize, Serialize};

pub type JrpcFutResult<T> = jrpc::BoxFuture<jrpc::Result<T>>;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DeleteBulkProgress {
    pub total: u64,
    pub deleted: u64,
    pub errors: u64,
}
