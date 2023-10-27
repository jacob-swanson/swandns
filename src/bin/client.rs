use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use cron_parser::parse;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use swandns::client::update_records;
use swandns::util::configure_tracing;
use swandns::{load_config, ClientConfig};
use tokio_graceful_shutdown::{FutureExt, SubsystemHandle, Toplevel};
use tracing::{debug, error, info};

static CONF_NAME: &str = "client";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config: Option<PathBuf>,
    #[arg(short, long)]
    schedule: Option<String>,
}

async fn update_in_loop(
    subsys: SubsystemHandle,
    cfg: Arc<ClientConfig>,
    schedule: String,
) -> Result<()> {
    let zero = Duration::new(0, 0);

    loop {
        if subsys.is_shutdown_requested() {
            return Ok(());
        }
        match update_records(cfg.clone())
            .cancel_on_shutdown(&subsys)
            .await
        {
            Ok(res) => match res {
                Ok(_) => {
                    info!("Records updated successfully");
                }
                Err(err) => {
                    error!("{:?}", err);
                }
            },
            Err(_) => {
                debug!("Cancelled by shutdown");
            }
        }

        let now = Utc::now();
        let upcoming = parse(schedule.as_str(), &now).unwrap();
        let duration = (upcoming - now).to_std()?;
        if duration > zero {
            info!("Waiting {:?} until next update", duration);
            if let Err(_) = tokio::time::sleep(duration)
                .cancel_on_shutdown(&subsys)
                .await
            {
                return Ok(());
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    configure_tracing();

    let args: Args = Args::parse();
    let cfg: Arc<ClientConfig> = Arc::new(load_config(CONF_NAME, args.config).await?);

    if let Some(schedule) = args.schedule {
        return Toplevel::new()
            .start("Schedule", |h| update_in_loop(h, cfg, schedule))
            .catch_signals()
            .handle_shutdown_requests(Duration::from_millis(1000))
            .await
            .map_err(Into::into);
    } else {
        update_records(cfg.clone()).await?;
    }
    Ok(())
}
