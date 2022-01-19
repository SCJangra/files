use crate::{file, utils};
use futures::TryFutureExt;
use jsonrpc_core as jrpc;
use jsonrpc_pubsub::{self as ps, typed as pst};
use std::collections as cl;
use tokio::{sync, task};
pub use types::*;

mod types;

lazy_static::lazy_static! {
    pub static ref ACTIVE: sync::RwLock<cl::HashMap<ps::SubscriptionId, task::JoinHandle<()>>> =
        sync::RwLock::new(cl::HashMap::<
            ps::SubscriptionId,
            task::JoinHandle<()>,
        >::new());
}

pub async fn list(id: file::FileId, task_id: ps::SubscriptionId, sink: pst::Sink<TaskResult>) {
    ACTIVE.write().await.insert(
        task_id.clone(),
        task::spawn(async move {
            let files = file::list_meta(&id)
                .map_ok_or_else(
                    |e| sink.notify(Err(utils::to_rpc_err(e))),
                    |v| sink.notify(Ok(TaskResult::ListResult(v))),
                )
                .await;
            if let Err(_e) = files {
                // TODO: Log this error
            }

            {
                ACTIVE.write().await.remove(&task_id);
            }
        }),
    );
}

pub async fn cancel_task(id: ps::SubscriptionId) -> jrpc::Result<bool> {
    let removed = ACTIVE.write().await.remove(&id);
    if let Some(h) = removed {
        h.abort();
        Ok(true)
    } else {
        Err(jrpc::Error {
            code: jrpc::ErrorCode::InvalidParams,
            message: "Invalid subscription.".into(),
            data: None,
        })
    }
}
