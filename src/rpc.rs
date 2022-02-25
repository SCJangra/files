mod fun;
mod types;

use files::{file, utils};
use fun::*;
use futures::{StreamExt, TryFutureExt};
use jsonrpc_core as jrpc;
use jsonrpc_pubsub::{self as ps, typed as pst};
use tokio::task;
use tokio_stream::wrappers as tsw;
use types::*;

pub struct RpcImpl;

#[jsonrpc_derive::rpc(server)]
pub trait Rpc {
    type Metadata;

    #[rpc(name = "get_meta")]
    fn get_meta(&self, id: file::FileId) -> JrpcFutResult<file::FileMeta>;

    #[rpc(name = "list")]
    fn list(&self, dir: file::FileId) -> JrpcFutResult<Vec<file::FileMeta>>;

    #[rpc(name = "list_all")]
    fn list_all(&self, dir: file::FileId) -> JrpcFutResult<Vec<file::FileMeta>>;

    #[rpc(name = "create")]
    fn create(
        &self,
        ft: file::FileType,
        name: String,
        dir: file::FileId,
    ) -> JrpcFutResult<file::FileId>;

    #[rpc(name = "delete")]
    fn delete(&self, ft: file::FileType, id: file::FileId) -> JrpcFutResult<()>;

    #[rpc(name = "rename")]
    fn rename(&self, file: file::FileId, new_name: String) -> JrpcFutResult<file::FileId>;

    #[rpc(name = "move_file")]
    fn move_file(&self, file: file::FileId, dir: file::FileId) -> JrpcFutResult<file::FileId>;

    #[pubsub(subscription = "copy_file", subscribe, name = "copy_file")]
    fn copy_file(
        &self,
        m: Self::Metadata,
        sub: pst::Subscriber<Option<Progress>>,
        source: file::FileId,
        dest: file::FileId,
        prog_interval: Option<u128>,
    );

    #[pubsub(subscription = "copy_file", unsubscribe, name = "copy_file_c")]
    fn copy_file_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;

    #[pubsub(subscription = "walk", subscribe, name = "walk")]
    fn walk(
        &self,
        m: Self::Metadata,
        sub: pst::Subscriber<Option<file::FileMeta>>,
        dir: file::FileId,
    );

    #[pubsub(subscription = "walk", unsubscribe, name = "walk_c")]
    fn walk_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;

    #[pubsub(subscription = "delete_bulk", subscribe, name = "delete_bulk")]
    fn delete_bulk(
        &self,
        m: Self::Metadata,
        sub: pst::Subscriber<Option<Progress>>,
        ft: file::FileType,
        ids: Vec<file::FileId>,
        prog_interval: Option<u128>,
    );

    #[pubsub(subscription = "delete_bulk", unsubscribe, name = "delete_bulk_c")]
    fn delete_bulk_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;
}

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

    fn list_all(&self, id: file::FileId) -> JrpcFutResult<Vec<file::FileMeta>> {
        Box::pin(async move {
            let files = tsw::UnboundedReceiverStream::new(file::walk(&id))
                .filter_map(|res| async move {
                    match res {
                        Ok(m) => Some(m),
                        Err(_e) => None, // TODO: Log this error
                    }
                })
                .collect::<Vec<file::FileMeta>>()
                .await;
            Ok(files)
        })
    }

    fn create(
        &self,
        ft: file::FileType,
        name: String,
        dir: file::FileId,
    ) -> JrpcFutResult<file::FileId> {
        Box::pin(async move {
            use file::FileType::*;
            let id = match ft {
                File => file::create_file(&name, &dir).await,
                Dir => file::create_dir(&name, &dir).await,
                Unknown => {
                    let err = anyhow::anyhow!("Unsupported operation!");
                    return Err(utils::to_rpc_err(err));
                }
            }
            .map_err(utils::to_rpc_err)?;
            Ok(id)
        })
    }

    fn delete(&self, ft: file::FileType, id: file::FileId) -> JrpcFutResult<()> {
        Box::pin(async move {
            use file::FileType::*;
            match ft {
                Dir => file::delete_dir(&id).await,
                _ => file::delete_file(&id).await,
            }
            .map_err(utils::to_rpc_err)?;
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
        sub: pst::Subscriber<Option<Progress>>,
        source: file::FileId,
        dest: file::FileId,
        prog_interval: Option<u128>,
    ) {
        task::spawn(async move {
            let (task_id, sink) = get_sink(sub)
                .inspect_err(|_e| { /* TODO: Log this error */ })
                .await?;

            ACTIVE.write().await.insert(
                task_id.clone(),
                task::spawn(async move {
                    let prog_interval = prog_interval.unwrap_or(1000);

                    let res = copy_file(&source, &dest, &sink, prog_interval).await;

                    if let Err(_e) = res {
                        // TODO: Log this error
                    }

                    {
                        ACTIVE.write().await.remove(&task_id);
                    }
                }),
            );

            anyhow::Ok(())
        });
    }

    fn copy_file_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> JrpcFutResult<bool> {
        Box::pin(cancel_sub(id))
    }

    fn walk(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<Option<file::FileMeta>>,
        dir: file::FileId,
    ) {
        task::spawn(async move {
            let (task_id, sink) = get_sink(sub)
                .inspect_err(|_e| { /* TODO: Log this error */ })
                .await?;

            ACTIVE.write().await.insert(
                task_id.clone(),
                task::spawn(async move {
                    let res = walk(&dir, &sink).await;

                    if let Err(_e) = res {
                        // TODO: Log this error
                    }

                    {
                        ACTIVE.write().await.remove(&task_id);
                    }
                }),
            );

            anyhow::Ok(())
        });
    }

    fn walk_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>> {
        Box::pin(cancel_sub(id))
    }

    fn delete_bulk(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<Option<Progress>>,
        ft: file::FileType,
        ids: Vec<file::FileId>,
        prog_interval: Option<u128>,
    ) {
        task::spawn(async move {
            let (task_id, sink) = get_sink(sub)
                .inspect_err(|_e| { /* TODO: Log this error */ })
                .await?;

            ACTIVE.write().await.insert(
                task_id.clone(),
                task::spawn(async move {
                    let prog_interval = prog_interval.unwrap_or(1000);
                    let res = delete_bulk(&ft, &ids, &sink, prog_interval).await;

                    if let Err(_e) = res {
                        // TODO: Log this error
                    }

                    {
                        ACTIVE.write().await.remove(&task_id);
                    }
                }),
            );

            anyhow::Ok(())
        });
    }

    fn delete_bulk_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>> {
        Box::pin(cancel_sub(id))
    }
}
