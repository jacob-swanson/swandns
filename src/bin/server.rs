use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use swandns::dns_server::DnsServer;
use swandns::record_repository::RecordRepository;
use swandns::rpc_server::RpcServer;
use swandns::util::{configure_tracing, get_socket_addr, migrate_database, open_database};
use swandns::{load_config, ServerConfig};
use tokio_graceful_shutdown::{FutureExt, SubsystemHandle, Toplevel, SubsystemBuilder};
use tracing::debug;

static CONF_NAME: &str = "server";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config: Option<PathBuf>,
}

async fn start_dns_server(
    subsys: SubsystemHandle,
    cfg: Arc<ServerConfig>,
    repo: Arc<RecordRepository>,
) -> Result<()> {
    let dns_server = DnsServer { repo, cfg };
    if let Err(_) = dns_server.run().cancel_on_shutdown(&subsys).await {
        debug!("DNS server shutdown");
    }
    Ok(())
}

async fn start_rpc_server(
    subsys: SubsystemHandle,
    cfg: Arc<ServerConfig>,
    repo: Arc<RecordRepository>,
) -> Result<()> {
    let listen_addr: SocketAddr =
        get_socket_addr(cfg.bind.clone(), cfg.api_port, Some("ipv4".to_string()))?;
    let rpc_server = RpcServer {
        addr: listen_addr,
        repo,
    };
    if let Err(_) = rpc_server.run().cancel_on_shutdown(&subsys).await {
        debug!("DNS server shutdown");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    configure_tracing();

    let args: Args = Args::parse();

    let cfg: Arc<ServerConfig> = Arc::new(load_config(CONF_NAME, args.config).await?);
    let dns_cfg = cfg.clone();
    let rpc_cfg = cfg.clone();

    let conn = Arc::new(open_database(&cfg.data_dir, &cfg.db_file).await?);
    let record_repo = Arc::new(RecordRepository { conn: conn.clone() });
    let dns_repo = record_repo.clone();
    let rpc_repo = record_repo.clone();

    migrate_database(conn.clone()).await?;

    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("DnsServer", |h| start_dns_server(h, dns_cfg, dns_repo)));
        s.start(SubsystemBuilder::new("RpcServer", |h| start_rpc_server(h, rpc_cfg, rpc_repo)));
    })
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
}
