use std::sync::Arc;
use std::time::Duration;
use swandns::client::update_record;
use swandns::proto::records_client::RecordsClient;
use swandns::proto::{FindUniqueRecordRequest, RecordReply, RecordsQueryRequest};
use swandns::record_repository::RecordRepository;
use swandns::rpc_server::RpcServer;
use swandns::util::{configure_tracing, migrate_database};
use swandns::{ClientConfig, ClientRecordConfig};
use tokio_rusqlite::Connection;
use tokio_stream::StreamExt;
use tonic::Code;

#[tokio::test]
async fn test_rpc_server() {
    configure_tracing();

    // Start server
    let conn = Arc::new(Connection::open_in_memory().await.unwrap());
    migrate_database(conn.clone()).await.unwrap();
    let repo = Arc::new(RecordRepository { conn });
    let rpc_server = Arc::new(RpcServer {
        addr: "127.0.0.1:8080".parse().unwrap(),
        repo,
    });
    let rpc_server_fut = tokio::spawn(async move { rpc_server.run().await });

    // Wait for server to start
    tokio::time::sleep(Duration::from_secs(1)).await;

    let server_url = "http://127.0.0.1:8080";
    let mut client = RecordsClient::connect(server_url).await.unwrap();

    // Create
    let record = update_record(
        Arc::new(Default::default()),
        ClientRecordConfig {
            server_url: Some(server_url.to_string()),
            name: "example.com".to_string(),
            bind: Some("lo".to_string()),
            protocol: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(record.name, "example.com");
    assert_eq!(record.data, "127.0.0.1");
    assert_eq!(record.r#type, "A");
    assert_eq!(record.ttl, 30);
    assert_eq!(record.healthy, true);
    assert!(record.created_at > 0);
    assert!(record.updated_at > 0);

    // Create w/ fallbacks
    let record = update_record(
        Arc::new(ClientConfig {
            default_server_url: Some(server_url.to_string()),
            default_bind: Some("lo".to_string()),
            default_protocol: Some("ipv6".to_string()),
            ..Default::default()
        }),
        ClientRecordConfig {
            server_url: None,
            name: "example.com".to_string(),
            bind: None,
            protocol: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(record.name, "example.com");
    assert_eq!(record.data, "::1");
    assert_eq!(record.r#type, "AAAA");
    assert_eq!(record.ttl, 30);
    assert_eq!(record.healthy, true);
    assert!(record.created_at > 0);
    assert!(record.updated_at > 0);

    // Find unique call
    let record = client
        .find_unique(FindUniqueRecordRequest {
            name: "example.com".to_string(),
            r#type: "AAAA".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(record.name, "example.com");
    assert_eq!(record.data, "::1");
    assert_eq!(record.r#type, "AAAA");

    // Not found call
    let status = client
        .find_unique(FindUniqueRecordRequest {
            name: "google.com".to_string(),
            r#type: "A".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::NotFound);

    // List call
    let mut stream = client
        .list(RecordsQueryRequest {})
        .await
        .unwrap()
        .into_inner();
    let mut records: Vec<RecordReply> = vec![];
    while let Some(record) = stream.next().await {
        let record = record.unwrap();
        records.push(record);
    }
    assert_eq!(records.len(), 2);

    // Delete call
    let _response = client
        .delete(FindUniqueRecordRequest {
            name: "example.com".to_string(),
            r#type: "A".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    // Verify delete
    let mut stream = client
        .list(RecordsQueryRequest {})
        .await
        .unwrap()
        .into_inner();
    let mut records: Vec<RecordReply> = vec![];
    while let Some(record) = stream.next().await {
        let record = record.unwrap();
        records.push(record);
    }
    assert_eq!(records.len(), 1);

    rpc_server_fut.abort();
}
