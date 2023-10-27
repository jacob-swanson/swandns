use crate::proto::records_client::RecordsClient;
use crate::proto::{RecordReply, UpsertRecordRequest};
use crate::util::get_iface_addr;
use crate::{ClientConfig, ClientRecordConfig};
use anyhow::{anyhow, Result};
use std::iter::Iterator;
use std::sync::Arc;
use tokio_retry::strategy::{jitter, FibonacciBackoff};
use tokio_retry::Retry;
use tonic::{Request, Response};
use tracing::{debug, info, warn};

async fn client_upsert(
    server_url: String,
    message: &UpsertRecordRequest,
) -> Result<Response<RecordReply>> {
    let mut client = RecordsClient::connect(server_url).await?;
    let res = client.upsert(Request::new(message.clone())).await?;
    Ok(res)
}

pub async fn update_record(
    cfg: Arc<ClientConfig>,
    record_config: ClientRecordConfig,
) -> Result<RecordReply> {
    let name = record_config.name;
    let bind = record_config.bind.or(cfg.default_bind.clone());
    let protocol = record_config.protocol.or(cfg.default_protocol.clone());
    let ip_addr = get_iface_addr(bind, protocol)?;
    let server_url = record_config
        .server_url
        .clone()
        .or(cfg.default_server_url.clone())
        .unwrap_or("http://127.0.0.1:8080".to_string());

    debug!("Sending {:?}={:?} to {:?}", name, ip_addr, server_url);

    let r#type = if ip_addr.is_ipv6() { "AAAA" } else { "A" };
    let message = UpsertRecordRequest {
        name: name.clone(),
        r#type: r#type.to_string(),
        value: ip_addr.to_string(),
        ttl: 30,
    };
    let retry_policy = FibonacciBackoff::from_millis(1000).map(jitter).take(5);
    let res = Retry::spawn(retry_policy, || {
        client_upsert(server_url.to_string(), &message)
    })
    .await?;
    debug!("Response: {:?}", res);
    let reply = res.into_inner();

    info!("Updated {:?} to {:?}", name, ip_addr);
    Ok(reply)
}

pub async fn update_records(cfg: Arc<ClientConfig>) -> Result<Vec<RecordReply>> {
    let result: Vec<RecordReply> = vec![];
    if cfg.records.is_empty() {
        warn!("No update records configured");
        return Ok(result);
    }

    let mut errors = 0;
    for record in cfg.records.clone().into_iter() {
        if let Err(err) = update_record(cfg.clone(), record.clone()).await {
            errors += 1;
            warn!("There was a problem updating {}: {}", record.name, err);
        }
    }

    if errors > 0 {
        return Err(anyhow!("{} records not updated", errors));
    }
    Ok(result)
}
