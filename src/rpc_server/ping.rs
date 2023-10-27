use crate::proto::ping_server::Ping;
use crate::proto::{PingReply, PingRequest};
use tonic::{Request, Response, Status};
use tracing::info;

#[derive(Debug, Default)]
pub struct MyPing {}

impl MyPing {
    pub fn new() -> MyPing {
        Self {}
    }
}

#[tonic::async_trait]
impl Ping for MyPing {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingReply>, Status> {
        info!("Got a request: {:?}", request);
        let reply = PingReply {
            message: "pong".to_string(),
        };
        Ok(Response::new(reply))
    }
}
