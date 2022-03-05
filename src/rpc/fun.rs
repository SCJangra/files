use files::{
    file::{self, FileMeta},
    notify, notify_err, notify_ok, utils,
};
use futures::{self as futs, StreamExt, TryStreamExt};
use jsonrpc_core as jrpc;
use jsonrpc_pubsub::{self as ps, manager::IdProvider, typed as pst};
use std::{collections as cl, time};
use tokio::{sync, task};
use tokio_stream::wrappers as tsw;

use super::types::*;

lazy_static::lazy_static! {
    pub static ref ACTIVE: sync::RwLock<cl::HashMap<ps::SubscriptionId, task::JoinHandle<()>>> =
        sync::RwLock::new(cl::HashMap::<
            ps::SubscriptionId,
            task::JoinHandle<()>
        >::new());
    static ref RAND_STR_ID: ps::manager::RandomStringIdProvider =
        ps::manager::RandomStringIdProvider::new();
}

pub async fn get_sink<T>(
    sub: pst::Subscriber<T>,
) -> anyhow::Result<(ps::SubscriptionId, pst::Sink<T>)> {
    let task_id = ps::SubscriptionId::String(RAND_STR_ID.next_id());
    let sink = sub
        .assign_id_async(task_id.clone())
        .await
        .map_err(|_| anyhow::anyhow!("Could not subscribe!"))?;

    Ok((task_id, sink))
}

pub async fn run<Fut, Fun, T>(sub: pst::Subscriber<T>, fun: Fun)
where
    Fut: futs::Future<Output = ()> + Send + 'static,
    Fun: FnOnce(pst::Sink<T>) -> Fut + Send + Sync + 'static,
    T: Send + 'static,
{
    let (sub_id, sink) = match get_sink(sub).await {
        Err(_e) => {
            /* TODO: Log this error */
            return;
        }
        Ok(v) => v,
    };

    ACTIVE.write().await.insert(
        sub_id.clone(),
        task::spawn(async move {
            fun(sink).await;

            {
                ACTIVE.write().await.remove(&sub_id);
            }
        }),
    );
}

pub async fn sub_c(id: ps::SubscriptionId) -> jrpc::Result<bool> {
    let removed = ACTIVE.write().await.remove(&id);
    if let Some(r) = removed {
        r.abort();
        Ok(true)
    } else {
        Err(jrpc::Error {
            code: jrpc::ErrorCode::InvalidParams,
            message: "Invalid subscription.".into(),
            data: None,
        })
    }
}

pub async fn copy_file(
    sink: pst::Sink<Option<file::Progress>>,
    file: file::FileId,
    dst_dir: file::FileId,
    prog_interval: Option<u128>,
) -> anyhow::Result<()> {
    let r = match file::copy_file(&file, &dst_dir).await {
        Err(e) => {
            notify_err!(sink, utils::to_rpc_err(e))?;
            return Ok(());
        }
        Ok(r) => r,
    };

    let prog_interval = prog_interval.unwrap_or(1000);
    let mut instant = time::Instant::now();

    tsw::UnboundedReceiverStream::new(r)
        .map(|res| match res {
            Err(e) => notify_err!(sink, utils::to_rpc_err(e)),
            Ok(p) => {
                let is_done = p.total <= p.done;
                if instant.elapsed().as_millis() < prog_interval && !is_done {
                    return Ok(());
                }
                instant = time::Instant::now();
                notify_ok!(sink, Some(p))
            }
        })
        .try_for_each(|_| async { Ok(()) })
        .await
        .and_then(|_| notify_ok!(sink, None))?;

    Ok(())
}

pub async fn dfs(sink: pst::Sink<Option<FileMeta>>, id: file::FileId) -> anyhow::Result<()> {
    let r = file::dfs(&id).await?;

    tsw::UnboundedReceiverStream::new(r)
        .map(|res| match res {
            Ok(m) => notify_ok!(sink, Some(m)),
            Err(e) => notify_err!(sink, utils::to_rpc_err(e)),
        })
        .try_for_each(|_| async { anyhow::Ok(()) })
        .await
        .and_then(|_| notify_ok!(sink, None))?;

    anyhow::Ok(())
}

pub async fn delete_file_bulk(
    sink: pst::Sink<Option<DeleteBulkProgress>>,
    files: Vec<file::FileId>,
    prog_interval: Option<u128>,
) -> anyhow::Result<()> {
    let mut prog = DeleteBulkProgress {
        total: files.len() as u64,
        ..Default::default()
    };

    let prog_interval = prog_interval.unwrap_or(1000);

    let instant = time::Instant::now();

    futs::stream::iter(files)
        .map(|f| async move { file::delete_file(&f).await })
        .buffer_unordered(1000)
        .map(|r| {
            let r = match r {
                Ok(_) => {
                    prog.deleted += 1;
                    Ok(Some(prog.clone()))
                }
                Err(e) => {
                    prog.errors += 1;
                    Err(utils::to_rpc_err(e))
                }
            };

            let is_done = (prog.errors + prog.deleted) == prog.total;
            if instant.elapsed().as_millis() < prog_interval && !is_done {
                return Ok(());
            }

            notify!(sink, r)
        })
        .try_for_each(|_| async { anyhow::Ok(()) })
        .await
        .and_then(|_| notify_ok!(sink, None))?;

    anyhow::Ok(())
}

pub async fn delete_dir_bulk(
    sink: pst::Sink<Option<DeleteBulkProgress>>,
    dirs: Vec<file::FileId>,
    prog_interval: Option<u128>,
) -> anyhow::Result<()> {
    let mut prog = DeleteBulkProgress {
        total: dirs.len() as u64,
        ..Default::default()
    };

    let prog_interval = prog_interval.unwrap_or(1000);

    let instant = time::Instant::now();

    futs::stream::iter(dirs)
        .then(|f| async move { file::delete_dir(&f).await })
        .map(|r| {
            let r = match r {
                Ok(_) => {
                    prog.deleted += 1;
                    Ok(Some(prog.clone()))
                }
                Err(e) => {
                    prog.errors += 1;
                    Err(utils::to_rpc_err(e))
                }
            };

            let is_done = (prog.errors + prog.deleted) == prog.total;
            if instant.elapsed().as_millis() < prog_interval && !is_done {
                return Ok(());
            }

            notify!(sink, r)
        })
        .try_for_each(|_| async { anyhow::Ok(()) })
        .await
        .and_then(|_| notify_ok!(sink, None))?;

    anyhow::Ok(())
}
