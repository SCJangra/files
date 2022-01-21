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
}
