use crate::proto::records_server::Records;
use crate::proto::{
    EmptyReply, FindUniqueRecordRequest, RecordReply, RecordsQueryRequest, UpsertRecordRequest,
};
use crate::record_repository::RecordRepository;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct MyRecords {
    pub repo: Arc<RecordRepository>,
}

#[tonic::async_trait]
impl Records for MyRecords {
    async fn find_unique(
        &self,
        request: Request<FindUniqueRecordRequest>,
    ) -> std::result::Result<Response<RecordReply>, Status> {
        match self.repo.find_unique(request.into_inner()).await {
            Ok(record) => Ok(Response::new(record)),
            Err(_) => Err(Status::not_found("record not found")),
        }
    }

    async fn upsert(
        &self,
        request: Request<UpsertRecordRequest>,
    ) -> Result<Response<RecordReply>, Status> {
        let record = self.repo.upsert(request.into_inner()).await.unwrap();
        Ok(Response::new(record))
    }

    type ListStream = ReceiverStream<Result<RecordReply, Status>>;

    async fn list(
        &self,
        _request: Request<RecordsQueryRequest>,
    ) -> Result<Response<Self::ListStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        let repo = self.repo.clone();
        tokio::spawn(async move {
            let records = repo.list().await.unwrap();
            for record in records {
                tx.send(Ok(record)).await.unwrap();
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn delete(
        &self,
        request: Request<FindUniqueRecordRequest>,
    ) -> Result<Response<EmptyReply>, Status> {
        self.repo.delete(request.into_inner()).await.unwrap();
        Ok(Response::new(EmptyReply {}))
    }
}
