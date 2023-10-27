mod ping;
mod records;

use crate::proto::ping_server::PingServer;
use crate::proto::records_server::RecordsServer;
use crate::record_repository::RecordRepository;
use anyhow::Result;
pub use ping::*;
pub use records::*;
use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;
use tracing::info;

pub struct RpcServer {
    pub addr: SocketAddr,
    pub repo: Arc<RecordRepository>,
}

impl RpcServer {
    pub async fn run(&self) -> Result<()> {
        info!("RPC server listening on: {:?}", self.addr);
        Server::builder()
            .add_service(PingServer::new(MyPing::new()))
            .add_service(RecordsServer::new(MyRecords {
                repo: self.repo.clone(),
            }))
            .serve(self.addr)
            .await?;
        Ok(())
    }
}
