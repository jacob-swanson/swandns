use anyhow::Result;
use hickory_client::client::{AsyncClient, ClientHandle};
use hickory_client::op::ResponseCode;
use hickory_client::proto::iocompat::AsyncIoTokioAsStd;
use hickory_client::rr::rdata::{A, AAAA};
use hickory_client::rr::{DNSClass, Name, RData, RecordType};
use hickory_client::tcp::TcpClientStream;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use swandns::dns_server::DnsServer;
use swandns::proto::UpsertRecordRequest;
use swandns::record_repository::RecordRepository;
use swandns::util::{configure_tracing, migrate_database};
use swandns::{RecordConfig, ServerConfig, ZoneConfig};
use tokio::net::TcpStream as TokioTcpStream;
use tokio_rusqlite::Connection;
use tracing::debug;

async fn create_client(socket_addr: SocketAddr) -> Result<AsyncClient> {
    let (stream, sender) = TcpClientStream::<AsyncIoTokioAsStd<TokioTcpStream>>::new(socket_addr);
    let (client, bg) = AsyncClient::new(stream, sender, None).await?;
    tokio::spawn(bg);
    Ok(client)
}

async fn test_query(
    client: &mut AsyncClient,
    name: &str,
    query_type: RecordType,
    expected_addr: &str,
) {
    let query = client.query(Name::from_str(name).unwrap(), DNSClass::IN, query_type);
    let response = query.await.unwrap();
    debug!("{:?}", response);
    assert_eq!(response.answers().len(), 1);
    match response.answers()[0].data() {
        Some(RData::A(addr)) => assert_eq!(*addr, A::from_str(expected_addr).unwrap()),
        Some(RData::AAAA(addr)) => assert_eq!(*addr, AAAA::from_str(expected_addr).unwrap()),
        _ => assert!(false),
    }
}

#[tokio::test]
async fn test_resolve_dns() {
    configure_tracing();

    let conn = Arc::new(Connection::open_in_memory().await.unwrap());
    migrate_database(conn.clone()).await.unwrap();
    let repo = Arc::new(RecordRepository { conn });
    let cfg: Arc<ServerConfig> = Arc::new(ServerConfig {
        nameservers: vec!["1.1.1.1".to_string()],
        zones: vec![
            ZoneConfig {
                name: "example.com".to_string(),
                records: vec![
                    RecordConfig {
                        key: "www".to_string(),
                        value: "127.0.0.1".to_string(),
                    },
                    RecordConfig {
                        key: "@".to_string(),
                        value: "127.0.0.2".to_string(),
                    },
                ],
            },
            ZoneConfig {
                name: "example.org".to_string(),
                records: vec![],
            },
        ],
        ..Default::default()
    });

    let dns_server = Arc::new(DnsServer {
        repo: repo.clone(),
        cfg,
    });
    let socket_addr = dns_server.get_socket_addr().unwrap();
    let dns_server_fut = tokio::spawn(async move { dns_server.run().await });

    // Wait for server to start
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut client = create_client(socket_addr).await.unwrap();

    // Record from config
    test_query(&mut client, "www.example.com", RecordType::A, "127.0.0.1").await;

    // Root record from "@" key
    test_query(&mut client, "example.com", RecordType::A, "127.0.0.2").await;

    // Record from DB
    repo.upsert(UpsertRecordRequest {
        name: "foo.example.com".to_string(),
        r#type: "A".to_string(),
        value: "127.0.0.3".to_string(),
        ttl: 30,
    })
    .await
    .unwrap();
    test_query(&mut client, "foo.example.com", RecordType::A, "127.0.0.3").await;

    // Upstream record
    let res = client
        .query(
            Name::from_str("example.org").unwrap(),
            DNSClass::IN,
            RecordType::A,
        )
        .await
        .unwrap();
    assert!(res.answers().len() > 0);

    // Reject unknown zone
    let res = client
        .query(
            Name::from_str("google.com").unwrap(),
            DNSClass::IN,
            RecordType::A,
        )
        .await
        .unwrap();
    assert_eq!(res.header().response_code(), ResponseCode::Refused);

    dns_server_fut.abort();
}
