use files::{file, notify_err, notify_ok, utils};
use futures::{self as futs, TryFutureExt};
use jsonrpc_core as jrpc;
use jsonrpc_pubsub::{self as ps, typed as pst};
use ps::manager::IdProvider;
use serde::{Deserialize, Serialize};
use std::collections as cl;
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt},
    sync, task,
};

type JrpcFutResult<T> = jrpc::BoxFuture<jrpc::Result<T>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct CopyFileProgress {
    total: u64,
    done: u64,
    percent: f64,
}

lazy_static::lazy_static! {
    pub static ref ACTIVE: sync::RwLock<cl::HashMap<ps::SubscriptionId, task::JoinHandle<anyhow::Result<()>>>> =
        sync::RwLock::new(cl::HashMap::<
            ps::SubscriptionId,
            task::JoinHandle<anyhow::Result<()>>,
        >::new());
    static ref RAND_STR_ID: ps::manager::RandomStringIdProvider =
        ps::manager::RandomStringIdProvider::new();
}

#[jsonrpc_derive::rpc(server)]
pub trait Rpc {
    type Metadata;

    #[rpc(name = "get_meta")]
    fn get_meta(&self, id: file::FileId) -> JrpcFutResult<file::FileMeta>;

    #[rpc(name = "list")]
    fn list(&self, dir: file::FileId) -> JrpcFutResult<Vec<file::FileMeta>>;

    #[rpc(name = "create_file")]
    fn create_file(&self, name: String, dir: file::FileId) -> JrpcFutResult<file::FileId>;

    #[rpc(name = "create_dir")]
    fn create_dir(&self, name: String, dir: file::FileId) -> JrpcFutResult<file::FileId>;

    #[rpc(name = "delete_file")]
    fn delete_file(&self, file: file::FileId) -> JrpcFutResult<()>;

    #[rpc(name = "delete_dir")]
    fn delete_dir(&self, dir: file::FileId) -> JrpcFutResult<()>;

    #[rpc(name = "rename")]
    fn rename(&self, file: file::FileId, new_name: String) -> JrpcFutResult<file::FileId>;

    #[rpc(name = "move_file")]
    fn move_file(&self, file: file::FileId, dir: file::FileId) -> JrpcFutResult<file::FileId>;

    #[pubsub(subscription = "copy_file", subscribe, name = "copy_file")]
    fn copy_file(
        &self,
        m: Self::Metadata,
        sub: pst::Subscriber<Option<CopyFileProgress>>,
        source: file::FileId,
        dest: file::FileId,
    );

    #[pubsub(subscription = "copy_file", unsubscribe, name = "copy_file_c")]
    fn copy_file_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;
}

pub struct RpcImpl;

impl Rpc for RpcImpl {
    type Metadata = std::sync::Arc<ps::Session>;

    fn get_meta(&self, id: file::FileId) -> JrpcFutResult<file::FileMeta> {
        Box::pin(async move {
            let m = file::get_meta(&id).await.map_err(utils::to_rpc_err)?;
            Ok(m)
        })
    }

    fn list(&self, dir: file::FileId) -> JrpcFutResult<Vec<file::FileMeta>> {
        Box::pin(async move {
            let f = file::list_meta(&dir).await.map_err(utils::to_rpc_err)?;
            Ok(f)
        })
    }

    fn create_file(&self, name: String, dir: file::FileId) -> JrpcFutResult<file::FileId> {
        Box::pin(async move {
            let id = file::create_file(&name, &dir)
                .await
                .map_err(utils::to_rpc_err)?;
            Ok(id)
        })
    }

    fn create_dir(&self, name: String, dir: file::FileId) -> JrpcFutResult<file::FileId> {
        Box::pin(async move {
            let id = file::create_dir(&name, &dir)
                .await
                .map_err(utils::to_rpc_err)?;
            Ok(id)
        })
    }

    fn delete_file(&self, file: file::FileId) -> JrpcFutResult<()> {
        Box::pin(async move {
            file::delete_file(&file).await.map_err(utils::to_rpc_err)?;
            Ok(())
        })
    }

    fn delete_dir(&self, dir: file::FileId) -> JrpcFutResult<()> {
        Box::pin(async move {
            file::delete_dir(&dir).await.map_err(utils::to_rpc_err)?;
            Ok(())
        })
    }

    fn rename(&self, file: file::FileId, new_name: String) -> JrpcFutResult<file::FileId> {
        Box::pin(async move {
            let id = file::rename(&file, &new_name)
                .await
                .map_err(utils::to_rpc_err)?;
            Ok(id)
        })
    }

    fn move_file(&self, file: file::FileId, dir: file::FileId) -> JrpcFutResult<file::FileId> {
        Box::pin(async move {
            let id = file::move_file(&file, &dir)
                .await
                .map_err(utils::to_rpc_err)?;
            Ok(id)
        })
    }

    fn copy_file(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<Option<CopyFileProgress>>,
        source: file::FileId,
        dest: file::FileId,
    ) {
        task::spawn(async move {
            let task_id = ps::SubscriptionId::String(RAND_STR_ID.next_id());
            let sink = sub
                .assign_id_async(task_id.clone())
                .inspect_err(|_e| { /* TODO: Log this error */ })
                .await?;

            ACTIVE.write().await.insert(
                task_id.clone(),
                task::spawn(async move {
                    copy_file(&source, &dest, &sink)
                        .inspect_err(|_e| { /* TODO: Log this error */ })
                        .await?;

                    {
                        ACTIVE.write().await.remove(&task_id);
                    }

                    anyhow::Ok(())
                }),
            );

            Ok::<(), ()>(())
        });
    }

    fn copy_file_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> JrpcFutResult<bool> {
        Box::pin(cancel_sub(id))
    }
}

async fn copy_file(
    source: &file::FileId,
    dest: &file::FileId,
    sink: &pst::Sink<Option<CopyFileProgress>>,
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
            Ok(l) if l == 0 => {
                notify_ok!(sink, None)?;
                break;
            }
            Ok(l) => l,
        };

        reader.consume(len as usize);

        done += len;

        let progress = CopyFileProgress {
            total,
            done,
            percent: (done as f64) / (total as f64),
        };

        notify_ok!(sink, Some(progress))?;
    }

    Ok(())
}

async fn cancel_sub(id: ps::SubscriptionId) -> jrpc::Result<bool> {
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
