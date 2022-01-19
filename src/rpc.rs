use files::{file, task as rpc_task};
use futures::TryFutureExt;
use jsonrpc_core as jrpc;
use jsonrpc_pubsub::{self as ps, typed as pst};
use ps::manager::IdProvider;
use tokio::task;

lazy_static::lazy_static! {
    static ref RAND_STR_ID: ps::manager::RandomStringIdProvider =
        ps::manager::RandomStringIdProvider::new();
}

#[jsonrpc_derive::rpc(server)]
pub trait Rpc {
    type Metadata;

    #[pubsub(subscription = "list", subscribe, name = "list")]
    fn list(&self, m: Self::Metadata, sub: pst::Subscriber<rpc_task::TaskResult>, id: file::FileId);

    #[pubsub(subscription = "list", unsubscribe, name = "cancel_list")]
    fn list_c(
        &self,
        m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>>;
}

pub struct RpcImpl;

impl RpcImpl {
    async fn get_sink(
        sub: pst::Subscriber<rpc_task::TaskResult>,
    ) -> anyhow::Result<(ps::SubscriptionId, pst::Sink<rpc_task::TaskResult>)> {
        let id = ps::SubscriptionId::String(RAND_STR_ID.next_id());
        let sink = sub
            .assign_id_async(id.clone())
            .await
            .map_err(|_| anyhow::anyhow!("Could not get a sink! Request already terminated!"))?;
        Ok((id, sink))
    }
}

impl Rpc for RpcImpl {
    type Metadata = std::sync::Arc<ps::Session>;

    fn list(
        &self,
        _m: Self::Metadata,
        sub: pst::Subscriber<rpc_task::TaskResult>,
        id: file::FileId,
    ) {
        task::spawn(async move {
            let (task_id, sink) = Self::get_sink(sub)
                .inspect_err(|_e| { /* TODO: Log this error */ })
                .await?;

            rpc_task::list(id, task_id, sink).await;

            Ok::<(), anyhow::Error>(())
        });
    }

    fn list_c(
        &self,
        _m: Option<Self::Metadata>,
        id: ps::SubscriptionId,
    ) -> jrpc::BoxFuture<jrpc::Result<bool>> {
        Box::pin(rpc_task::cancel_task(id))
    }
}
