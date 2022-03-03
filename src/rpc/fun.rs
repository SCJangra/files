use files::{
    file::{self, FileMeta},
    notify, notify_err, notify_ok, utils,
};
use futures::{self as futs, StreamExt, TryFutureExt, TryStreamExt};
use jsonrpc_core as jrpc;
use jsonrpc_pubsub::{self as ps, manager::IdProvider, typed as pst};
use std::{collections as cl, time};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt},
    sync, task,
};
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
    source: &file::FileId,
    dest: &file::FileId,
    sink: &pst::Sink<Option<Progress>>,
    prog_interval: u128,
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
    let mut instant = time::Instant::now();

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
            Ok(l) if l == 0 => {
                notify_ok!(sink, None)?;
                break;
            }
            Ok(l) => l,
        };

        reader.consume(len as usize);

        done += len;

        if instant.elapsed().as_millis() < prog_interval && done != total {
            continue;
        }

        instant = time::Instant::now();

        let progress = Progress {
            total,
            done,
            percent: (done as f64) / (total as f64),
        };

        notify_ok!(sink, Some(progress))?;
    }

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
        .await?;

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
