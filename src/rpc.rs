use files::{file, task as rpc_task};
use jsonrpc_core as jrpc;
use jsonrpc_pubsub::{self as ps, typed as pst};
use tokio::task;

#[jsonrpc_derive::rpc(server)]
pub trait Rpc {
    type Metadata;

    #[pubsub(subscription = "list", subscribe, name = "list")]
    fn list(&self, m: Self::Metadata, sub: pst::Subscriber<rpc_task::TaskResult>, id: file::FileId);

    #[pubsub(subscription = "list", unsubscribe, name = "list_c")]
    fn list_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;

    #[pubsub(subscription = "create", subscribe, name = "create")]
    fn create(
        &self,
        m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        name: String,
        dir: file::FileId,
    );

    #[pubsub(subscription = "create", unsubscribe, name = "create_c")]
    fn create_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;

    #[pubsub(subscription = "copy_file", subscribe, name = "copy_file")]
    fn copy_file(
        &self,
        m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        source: file::FileId,
        dest: file::FileId,
    );

    #[pubsub(subscription = "copy_file", unsubscribe, name = "copy_file_c")]
    fn copy_file_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;

    #[pubsub(subscription = "rename", subscribe, name = "rename")]
    fn rename(
        &self,
        m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        file: file::FileId,
        new_name: String,
    );

    #[pubsub(subscription = "rename", unsubscribe, name = "rename_c")]
    fn rename_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;

    #[pubsub(subscription = "move_file", subscribe, name = "move_file")]
    fn move_file(
        &self,
        m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        file: file::FileId,
        dir: file::FileId,
    );

    #[pubsub(subscription = "move_file", unsubscribe, name = "move_file_c")]
    fn move_file_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;

    #[pubsub(subscription = "delete", subscribe, name = "delete")]
    fn delete(
        &self,
        m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        file: file::FileId,
    );

    #[pubsub(subscription = "delete", unsubscribe, name = "delete_c")]
    fn delete_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;
}

pub struct RpcImpl;

impl Rpc for RpcImpl {
    type Metadata = std::sync::Arc<ps::Session>;

    fn list(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        id: file::FileId,
    ) {
        task::spawn(async move {
            let res = rpc_task::run_task(rpc_task::Task::List(id), sub).await;

            if let Err(_e) = res {
                // TODO: Log this error
            }
        });
    }

    fn list_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>> {
        Box::pin(rpc_task::cancel_task(id))
    }

    fn create(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        name: String,
        dir: file::FileId,
    ) {
        task::spawn(async {
            let res = rpc_task::run_task(rpc_task::Task::Create { name, dir }, sub).await;

            if let Err(_e) = res {
                // TODO: Log this error
            }
        });
    }

    fn create_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>> {
        Box::pin(rpc_task::cancel_task(id))
    }

    fn copy_file(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        source: file::FileId,
        dest: file::FileId,
    ) {
        task::spawn(async move {
            let res = rpc_task::run_task(rpc_task::Task::CopyFile { source, dest }, sub).await;

            if let Err(_e) = res {
                // TODO: Log this error
            }
        });
    }

    fn copy_file_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>> {
        Box::pin(rpc_task::cancel_task(id))
    }

    fn rename(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        file: file::FileId,
        new_name: String,
    ) {
        task::spawn(async move {
            let res = rpc_task::run_task(rpc_task::Task::Rename { file, new_name }, sub).await;

            if let Err(_e) = res {
                // TODO: Log this error
            }
        });
    }

    fn rename_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>> {
        Box::pin(rpc_task::cancel_task(id))
    }

    fn move_file(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        file: file::FileId,
        dir: file::FileId,
    ) {
        task::spawn(async move {
            let res = rpc_task::run_task(rpc_task::Task::MoveFile { file, dir }, sub).await;

            if let Err(_e) = res {
                // TODO: Log this error
            }
        });
    }

    fn move_file_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>> {
        Box::pin(rpc_task::cancel_task(id))
    }

    fn delete(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        file: file::FileId,
    ) {
        task::spawn(async move {
            let res = rpc_task::run_task(rpc_task::Task::Delete(file), sub).await;

            if let Err(_e) = res {
                // TODO: Log this error
            }
        });
    }

    fn delete_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>> {
        Box::pin(rpc_task::cancel_task(id))
    }
}
