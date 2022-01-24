use crate::{file, notify_err, notify_ok, utils};
use futures::{self as futs, FutureExt, TryFutureExt};
use jsonrpc_core as jrpc;
use jsonrpc_pubsub::{self as ps, typed as pst};
use ps::manager::IdProvider;
use std::collections as cl;
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt},
    sync, task,
};
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
                    list(&id, &sink)
                        .inspect_err(|_e| { /* TODO: Log this error */ })
                        .await?
                }
                Task::Create { name, dir } => {
                    create(&name, &dir, &sink)
                        .inspect_err(|_e| { /* TODO: Log this error */ })
                        .await?
                }
                Task::CopyFile { source, dest } => {
                    copy_file(&source, &dest, &sink)
                        .inspect_err(|_e| { /* TODO: Log this error */ })
                        .await?
                }
                Task::Rename { file, new_name } => {
                    rename(&file, &new_name, &sink)
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

async fn list(id: &file::FileId, sink: &pst::Sink<TaskResult>) -> anyhow::Result<()> {
    file::list_meta(id)
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
    name: &str,
    dir: &file::FileId,
    sink: &pst::Sink<TaskResult>,
) -> anyhow::Result<()> {
    let fut = match name.ends_with('/') {
        true => file::create_dir(name, dir).boxed(),
        false => file::create_file(name, dir).boxed(),
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

async fn copy_file(
    source: &file::FileId,
    dest: &file::FileId,
    sink: &pst::Sink<TaskResult>,
) -> anyhow::Result<()> {
    let res = futs::try_join!(file::get_meta(source), file::get_meta(dest));
    let (sm, dm) = match res {
        Err(e) => {
            notify_err!(sink, utils::to_rpc_err(e))?;
            return Ok(());
        }
        Ok(m) => m,
    };
    let (r, w) = match file::copy_file(source, dest).await {
        Err(e) => {
            notify_err!(sink, utils::to_rpc_err(e))?;
            return Ok(());
        }
        Ok(rw) => rw,
    };

    let mut reader = io::BufReader::new(r);
    let mut writer = io::BufWriter::new(w);
    let mut done = 0u64;
    let total = sm.size;

    loop {
        let res: anyhow::Result<u64> = reader
            .fill_buf()
            .map_err(|e| {
                anyhow::Error::new(e).context(format!("Error while reading file '{}'", sm.name))
            })
            .and_then(|buf| {
                writer
                    .write_all(buf)
                    .map_err(|e| {
                        anyhow::Error::new(e)
                            .context(format!("Error while writing to file '{}'", dm.name))
                    })
                    .map_ok(|_| buf.len() as u64)
            })
            .await;

        if let Err(e) = writer.flush().await {
            let e = anyhow::Error::new(e)
                .context(format!("Error while writing file to disk '{}'", dm.name));
            notify_err!(sink, utils::to_rpc_err(e))?;
            break;
        }

        let len = match res {
            Err(e) => {
                notify_err!(sink, utils::to_rpc_err(e))?;
                break;
            }
            Ok(l) if l == 0 => break,
            Ok(l) => l,
        };

        reader.consume(len as usize);

        done += len;

        let progress = CopyFileProgress {
            total,
            done,
            percent: (done as f64) / (total as f64),
        };

        notify_ok!(sink, TaskResult::CopyFileProgress(progress))?;
    }

    Ok(())
}

async fn rename(
    file: &file::FileId,
    new_name: &str,
    sink: &pst::Sink<TaskResult>,
) -> anyhow::Result<()> {
    file::rename(file, new_name)
        .map_ok_or_else(
            |e| notify_err!(sink, utils::to_rpc_err(e)),
            |id| notify_ok!(sink, TaskResult::Rename(id)),
        )
        .await?;

    Ok(())
}
