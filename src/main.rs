use rpc::Rpc;
use jsonrpc_core as jrpc;
use jsonrpc_ipc_server as ipc;
use jsonrpc_pubsub as ps;

mod rpc;

fn main() {
    let mut io = jrpc::MetaIoHandler::default();
    let rpc = rpc::RpcImpl;
    io.extend_with(rpc.to_delegate());

    let builder = ipc::ServerBuilder::with_meta_extractor(io, |request: &ipc::RequestContext| {
        std::sync::Arc::new(ps::Session::new(request.sender.clone()))
    });
    let server = builder.start("/tmp/files").expect("Couldn't open socket");
    server.wait();
}
