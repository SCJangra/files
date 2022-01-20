use crate::{file, utils};
use futures::{FutureExt, TryFutureExt};
use jsonrpc_core as jrpc;
use jsonrpc_pubsub::{self as ps, typed as pst};
use ps::manager::IdProvider;
use std::collections as cl;
use tokio::{sync, task};
pub use types::*;

mod types;

lazy_static::lazy_static! {
    pub static ref ACTIVE: sync::RwLock<cl::HashMap<ps::SubscriptionId, task::JoinHandle<anyhow::Result<()>>>> =
        sync::RwLock::new(cl::HashMap::<
            ps::SubscriptionId,
            task::JoinHandle<anyhow::Result<()>>,
        >::new());
    static ref RAND_STR_ID: ps::manager::RandomStringIdProvider =
        ps::manager::RandomStringIdProvider::new();
}

pub async fn run_task(task: Task, sub: pst::Subscriber<TaskResult>) -> anyhow::Result<()> {
    let task_id = ps::SubscriptionId::String(RAND_STR_ID.next_id());
    let sink = sub
        .assign_id_async(task_id.clone())
        .await
        .map_err(|_| anyhow::anyhow!("Could not get a sink! Request already terminated!"))?;

    ACTIVE.write().await.insert(
        task_id.clone(),
        task::spawn(async move {
            match task {
                Task::List(id) => {
                    list(id, sink)
                        .inspect_err(|_e| { /* TODO: Log this error */ })
                        .await?
                }
                Task::Create { name, dir } => {
                    create(name, dir, sink)
                        .inspect_err(|_e| { /* TODO: Log this error */ })
                        .await?
                }
            };

            {
                ACTIVE.write().await.remove(&task_id);
            }
            anyhow::Ok(())
        }),
    );

    Ok(())
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

async fn list(id: file::FileId, sink: pst::Sink<TaskResult>) -> anyhow::Result<()> {
    file::list_meta(&id)
        .map_ok_or_else(
            |e| {
                sink.notify(Err(utils::to_rpc_err(e)))
                    .map_err(|e| anyhow::anyhow!("Could not notify error '{}'", e))
            },
            |v| {
                sink.notify(Ok(TaskResult::List(v)))
                    .map_err(|e| anyhow::anyhow!("Could not notify result '{}'", e))
            },
        )
        .await?;
    Ok(())
}

async fn create(
    name: String,
    dir: file::FileId,
    sink: pst::Sink<TaskResult>,
) -> anyhow::Result<()> {
    let fut = match name.ends_with('/') {
        true => file::create_dir(&name, &dir).boxed(),
        false => file::create_file(&name, &dir).boxed(),
    };

    fut.map_ok_or_else(
        |e| {
            sink.notify(Err(utils::to_rpc_err(e)))
                .map_err(|e| anyhow::anyhow!("Could not notify error '{}'", e))
        },
        |id| {
            sink.notify(Ok(TaskResult::Create(id)))
                .map_err(|e| anyhow::anyhow!("Could not notify result '{}'", e))
        },
    )
    .await?;

    Ok(())
}
